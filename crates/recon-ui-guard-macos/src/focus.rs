//! Watching what a person at the Mac would notice while a `ui` operation runs.
//!
//! Every `ui` operation promises to leave the user's foreground work alone once its prerequisites
//! are established. The promise is about what happened *during* the operation, so the guard keeps
//! a continuous trace rather than a before-and-after pair: an application that is activated and
//! deactivated again, or a pointer that is warped and put back, leaves the Mac looking untouched
//! at the end. A background thread samples the frontmost application, the order of the windows in
//! front of the user, the active Space, and the pointer and input event streams every few
//! milliseconds, and compares each sample with the one before it.
//!
//! Sampling has a floor: a change that happens and is undone between two looks leaves both of
//! them identical. So an interaction, which sends input of its own, also listens to what the
//! workspace announces as it happens — an application becoming active, the user's Space changing
//! — on a thread that runs a run loop for the whole operation. The announcements are evidence in
//! the same trace, judged by the same rules as the looks.
//!
//! The guard only watches. It never activates, raises, makes key, warps the pointer, posts an
//! event, or puts back anything it saw change: a disturbance is a failure the caller reports, not
//! a state the guard repairs. Restoring focus would be a second disturbance, and “activate, then
//! restore” is one of the practices the contract names.
//!
//! It also refuses to guess. Evidence that names the process the request selected is attributed
//! to the operation; a change observed while the hardware was generating input is attributed to
//! the person using the Mac and is not interference; anything else is reported as observed and
//! unexplained, which fails the operation without blaming the target. When a dimension cannot be
//! watched at all, the guard fails before the operation runs rather than claiming a quiet desktop
//! it never saw.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use std::{fmt, mem};

use objc2_core_foundation::{
    CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGPoint,
};
use objc2_core_graphics::{
    CGEvent, CGEventSource, CGEventSourceStateID, CGEventType, CGWindowListCopyWindowInfo,
    CGWindowListOption, kCGNullWindowID, kCGWindowLayer, kCGWindowNumber, kCGWindowOwnerPID,
};
use serde_json::json;

use crate::private;
use crate::transient::{Notice, Notices};
use recon_core::error::{
    ErrorCode, Failure, InspectionStage, Interference, InterferenceEvidence, ReleaseState,
};
use recon_core::process::Pid;
use recon_window_server_macos::window::WindowId;

/// How long the guard waits between looks at the Mac.
///
/// One sample costs about two tenths of a millisecond, nearly all of it the window server's
/// list, so ten milliseconds buys a trace fine enough to catch an activation that is immediately
/// undone without measurably slowing the operation being watched.
const INTERVAL: Duration = Duration::from_millis(10);

/// Pointer movement, as the event stream reports it.
const POINTER_EVENTS: [CGEventType; 4] = [
    CGEventType::MouseMoved,
    CGEventType::LeftMouseDragged,
    CGEventType::RightMouseDragged,
    CGEventType::OtherMouseDragged,
];

/// The events that carry a person's intent somewhere: keys, buttons, and the scroll wheel.
const INPUT_EVENTS: [CGEventType; 6] = [
    CGEventType::KeyDown,
    CGEventType::FlagsChanged,
    CGEventType::LeftMouseDown,
    CGEventType::RightMouseDown,
    CGEventType::OtherMouseDown,
    CGEventType::ScrollWheel,
];

/// What one guarded operation is, for the evidence its disturbances carry.
pub struct Scope {
    /// The stage a detected disturbance is reported against.
    pub stage: InspectionStage,
    /// The instance the request selected, when it names one. Evidence about this process is
    /// attributable to the operation; evidence about any other process is not.
    pub target: Option<Pid>,
    /// An expected macOS setup prompt may interrupt the desktop; it does not excuse target
    /// activation, pointer movement, input redirection, or Space changes.
    pub setup_prompts_expected: bool,
}

/// Runs `operation` while the guard watches, and refuses to return a success the Mac contradicts.
///
/// Observed interference is reported even when the operation also fails. The error message
/// retains that failure's cause; no attempt is retried or its effects undone.
///
/// # Errors
///
/// [`ErrorCode::InterfaceUnavailable`] when a dimension of the contract cannot be watched on this
/// Mac, [`ErrorCode::UiInterference`] for an observed disturbance, or whatever `operation` itself
/// returned.
pub fn guarded<T>(
    scope: Scope,
    operation: impl FnOnce() -> Result<T, Failure>,
) -> Result<T, Failure> {
    let guard = Guard::start(scope)?;
    let result = operation();
    let observation = guard.finish();
    observation.resolve(result)
}

pub fn guarded_interaction<T>(
    scope: Scope,
    operation: impl FnOnce(Watch) -> Result<(T, ReleaseState), Failure>,
) -> Result<T, Failure> {
    let guard = Guard::start(scope)?;
    let watch = guard.watch();
    let result = operation(watch.clone());
    let deadline = Instant::now() + Duration::from_millis(300);
    while Instant::now() < deadline && !watch.interrupted() {
        thread::sleep(INTERVAL);
    }
    let observation = guard.finish();
    observation.resolve_interaction(result)
}

#[derive(Clone)]
pub struct Watch {
    interrupted: Arc<AtomicBool>,
}

impl Watch {
    pub fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::Acquire)
    }
}

/// A running watch over one operation. A guard that is dropped instead of finished stops the
/// watcher too, so an early return cannot leave a thread sampling the Mac forever.
pub struct Guard {
    scope: Scope,
    stop: Arc<AtomicBool>,
    interrupted: Arc<AtomicBool>,
    watcher: JoinHandle<Trace>,
}

impl Guard {
    /// Resolves what this Mac lets Recon watch, then starts watching.
    ///
    /// An interaction also subscribes to what the workspace announces, before the first look, so
    /// that the whole of the operation is heard as well as seen.
    ///
    /// # Errors
    ///
    /// [`ErrorCode::InterfaceUnavailable`] when the frontmost application or the active Space
    /// cannot be observed on this macOS version, or when an interaction's notification observers
    /// cannot be registered. An operation that cannot be watched cannot claim it respected the
    /// user's foreground work, so the guard fails before it runs.
    pub fn start(scope: Scope) -> Result<Self, Failure> {
        let probe = Probe::of_this_mac()?;
        let stop = Arc::new(AtomicBool::new(false));
        let interrupted = Arc::new(AtomicBool::new(false));
        let watching = Arc::clone(&stop);
        let watching_interference = Arc::clone(&interrupted);
        let interaction = scope.stage == InspectionStage::Interaction;
        // An interaction posts events into the session, and an activation or Space switch it
        // causes can be over before the next look. Listening starts before the baseline, so the
        // operation that cannot be heard fails before it sends anything.
        let notices = if interaction {
            Some(Notices::listening().map_err(|reason| {
                Dimension::Workspace.unwatchable(format!(
                    "Recon could not listen for {} ({reason}), so a ui interaction cannot report \
                     that it left the user's foreground work alone",
                    Dimension::Workspace
                ))
            })?)
        } else {
            None
        };
        let orders = Orders {
            probe,
            target: scope.target,
            interaction,
            setup_prompts_expected: scope.setup_prompts_expected,
            clock: Instant::now(),
            notices,
        };
        let (baseline_tx, baseline_rx) = std::sync::mpsc::channel::<Option<Dimension>>();
        let watcher = thread::Builder::new()
            .name("recon-no-focus".to_owned())
            .spawn(move || watch(&orders, &watching, &watching_interference, &baseline_tx))
            .map_err(|error| {
                Failure::internal(format!("the no-focus guard could not start watching: {error}"))
            })?;
        // Return only once the watcher has recorded a readable baseline. A baseline that could not
        // read one of the contract's dimensions fails closed before the operation runs, rather than
        // letting an effect happen in an unwatched gap and reporting it only afterward.
        match baseline_rx.recv() {
            Ok(None) => {}
            Ok(Some(dimension)) => {
                stop.store(true, Ordering::Release);
                let _ = watcher.join();
                return Err(dimension.unwatchable(format!(
                    "Recon could not read {dimension} before the operation, so it cannot report \
                     that the user's foreground work was left alone"
                )));
            }
            Err(_) => {
                stop.store(true, Ordering::Release);
                let _ = watcher.join();
                return Err(Failure::internal(
                    "the no-focus guard could not record a baseline before the operation",
                ));
            }
        }
        Ok(Self { scope, stop, interrupted, watcher })
    }

    fn watch(&self) -> Watch {
        Watch { interrupted: Arc::clone(&self.interrupted) }
    }

    /// Stops watching and collects what the trace recorded. The watcher takes one last look after
    /// the operation returned, so the trace covers the whole operation.
    pub fn finish(self) -> Observation {
        self.stop.store(true, Ordering::Release);
        Observation { scope: self.scope, trace: self.watcher.join().ok() }
    }
}

/// Everything the guard saw during one operation, and what it means for the result.
pub struct Observation {
    scope: Scope,
    /// What the watcher recorded, or `None` when the watcher itself stopped unexpectedly.
    trace: Option<Trace>,
}

impl Observation {
    /// The failure this operation must report instead of its success, if any.
    ///
    /// The guard reports; the caller decides. A disturbance Recon can attribute to a person using
    /// the Mac is recorded and not reported, because a concurrent user action is not the target's
    /// doing.
    pub fn outcome(&self) -> Option<Failure> {
        self.judged(self.scope.setup_prompts_expected)
    }

    fn judged(&self, setup_prompts_expected: bool) -> Option<Failure> {
        let Some(trace) = &self.trace else {
            return Some(Failure::internal(
                "the no-focus guard stopped watching, so this operation cannot report that it \
                 left the user's foreground work alone",
            ));
        };
        if let Some(gap) = trace.gap {
            return Some(gap.dimension.unwatchable(format!(
                "Recon lost sight of {} {} ms into the operation, so it cannot report that the \
                 user's foreground work was left alone",
                gap.dimension,
                gap.at.as_millis()
            )));
        }
        if setup_prompts_expected {
            // A setup dialog may change the frontmost process and window order. It does not
            // authorize target activation, Space switches, pointer motion, or input redirection.
            return trace
                .disturbances
                .iter()
                .filter(|disturbance| disturbance.blocks_operation(setup_prompts_expected))
                .min_by_key(|disturbance| (disturbance.attribution.rank(), disturbance.at))
                .map(|disturbance| disturbance.failure(&self.scope));
        }
        // The most pointed evidence first, and the earliest of equals: a later disturbance is
        // usually a consequence of the first one.
        let reportable = trace
            .disturbances
            .iter()
            .filter(|disturbance| disturbance.attribution.reportable())
            .min_by_key(|disturbance| (disturbance.attribution.rank(), disturbance.at))?;
        Some(reportable.failure(&self.scope))
    }

    fn resolve<T>(&self, result: Result<T, Failure>) -> Result<T, Failure> {
        // An operation that failed for a missing grant asked macOS to request it, and the request
        // dialog is a setup prompt like the ones a scope can expect.
        let prompted = self.scope.setup_prompts_expected
            || matches!(&result, Err(error) if error.code == ErrorCode::PermissionDenied);
        match (result, self.judged(prompted)) {
            (Ok(value), None) => Ok(value),
            (Err(error), None) => Err(error),
            (Ok(_), Some(interference)) => Err(interference),
            (Err(error), Some(mut interference)) => {
                interference.message = format!(
                    "{}; the operation also failed with {:?}: {}",
                    interference.message, error.code, error.message
                );
                Err(interference)
            }
        }
    }
    fn resolve_interaction<T>(
        &self,
        result: Result<(T, ReleaseState), Failure>,
    ) -> Result<T, Failure> {
        let release = match &result {
            Ok((_, release)) => serde_json::to_value(release).expect("release state serializes"),
            Err(error) => error
                .details
                .as_ref()
                .and_then(|details| details.get("release"))
                .cloned()
                .unwrap_or_else(|| json!("not-needed")),
        };
        let mut resolved = self.resolve(result.map(|(value, _)| value));
        if let Err(error) = &mut resolved
            && error.code == ErrorCode::UiInterference
        {
            error.details.get_or_insert_with(|| json!({}))["release"] = release;
        }
        resolved
    }
}

/// What the watcher recorded, in the order it saw it.
#[derive(Default)]
struct Trace {
    samples: usize,
    disturbances: Vec<Disturbance>,
    /// The first moment a dimension could not be read. One is enough: from there on, the guard
    /// cannot report an undisturbed desktop.
    gap: Option<Gap>,
}

impl Trace {
    fn record(&mut self, sample: &Snapshot) {
        self.samples += 1;
        if self.gap.is_none()
            && let Some(dimension) = sample.unreadable()
        {
            self.gap = Some(Gap { dimension, at: sample.at });
        }
    }
}

#[derive(Clone, Copy)]
struct Gap {
    dimension: Dimension,
    at: Duration,
}

/// One thing the no-focus contract names, and the interface Recon watches it through.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Dimension {
    Foreground,
    KeyWindow,
    WindowOrder,
    Space,
    Pointer,
    /// What the workspace announces as it happens, which is the only account of a change that
    /// was undone between two looks.
    Workspace,
}

impl Dimension {
    fn interface(self) -> &'static str {
        match self {
            Self::Foreground => "no-focus-foreground",
            Self::KeyWindow => "frontmost-key-window",
            Self::WindowOrder => "no-focus-window-order",
            Self::Space => "no-focus-space",
            Self::Pointer => "no-focus-pointer",
            Self::Workspace => "no-focus-workspace-notifications",
        }
    }

    fn symbol(self) -> &'static str {
        match self {
            Self::Foreground => "_SLPSGetFrontProcess",
            Self::KeyWindow => "AXFocusedWindow",
            Self::WindowOrder => "CGWindowListCopyWindowInfo",
            Self::Space => "CGSGetActiveSpace",
            Self::Pointer => "CGEventCreate",
            Self::Workspace => "NSWorkspaceDidActivateApplicationNotification",
        }
    }

    fn unwatchable(self, message: String) -> Failure {
        Failure::new(
            ErrorCode::InterfaceUnavailable,
            message,
            json!({ "interface": self.interface(), "symbol": self.symbol() }),
        )
    }

    /// The failure for a dimension this macOS version does not expose at all.
    fn missing(self) -> Failure {
        self.unwatchable(format!(
            "Recon cannot watch {self} on this macOS version ({} not found), so a ui operation \
             cannot report that it left the user's foreground work alone",
            self.symbol()
        ))
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Foreground => "the frontmost application",
            Self::KeyWindow => "the frontmost application's key window",
            Self::WindowOrder => "the order of the windows on screen",
            Self::Space => "the active Space",
            Self::Pointer => "the pointer",
            Self::Workspace => "what the workspace announces about activations and Spaces",
        })
    }
}

/// One observed change in what a person at the Mac would notice.
struct Disturbance {
    evidence: InterferenceEvidence,
    attribution: Attribution,
    at: Duration,
    pid: Option<Pid>,
    window: Option<WindowId>,
    /// The whole observation as a sentence, including who the evidence points at. It becomes both
    /// the failure's message and its reported disturbance, so it has to stand on its own.
    detail: String,
}

impl Disturbance {
    fn new(
        evidence: InterferenceEvidence,
        attribution: Attribution,
        at: Duration,
        observed: &str,
    ) -> Self {
        let detail =
            format!("{observed} {} ms into the operation{}", at.as_millis(), attribution.clause());
        Self { evidence, attribution, at, pid: None, window: None, detail }
    }

    fn about(mut self, pid: Pid, window: Option<WindowId>) -> Self {
        self.pid = Some(pid);
        self.window = window;
        self
    }

    fn blocks_operation(&self, setup_prompts_expected: bool) -> bool {
        self.attribution.reportable()
            && (!setup_prompts_expected
                || self.attribution == Attribution::Target
                || !matches!(
                    self.evidence,
                    InterferenceEvidence::ForegroundApplication | InterferenceEvidence::KeyWindow
                ))
    }

    fn failure(&self, scope: &Scope) -> Failure {
        let mut interference =
            Interference::new(scope.stage, self.evidence).disturbance(self.detail.as_str());
        if let Some(pid) = self.pid {
            interference = interference.pid(pid.0);
        }
        if let Some(window) = self.window {
            interference = interference.window(window.0);
        }
        if self.attribution == Attribution::Target {
            // The target ran code that disturbed the desktop, so its work may still be running.
            // Recon neither unwinds it nor takes the focus back.
            interference = interference.target_execution_may_continue();
        }
        Failure::new(ErrorCode::UiInterference, self.detail.clone(), interference.details())
    }
}

/// Who the evidence points at. Recon blames the selected target only when the evidence names it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Attribution {
    /// The change names the process the request selected.
    Target,
    /// The hardware was generating input in the same interval: someone was using the Mac. A
    /// person's own switch of applications is not the target's doing and not a failed operation.
    ConcurrentUser,
    /// Observed, with nothing tying it either to the selected target or to the person at the
    /// keyboard. Recon cannot report that the desktop was left alone, and does not claim the
    /// target did it.
    Unexplained,
}

impl Attribution {
    fn reportable(self) -> bool {
        !matches!(self, Self::ConcurrentUser)
    }

    fn rank(self) -> u8 {
        match self {
            Self::Target => 0,
            Self::Unexplained => 1,
            Self::ConcurrentUser => 2,
        }
    }

    fn clause(self) -> &'static str {
        match self {
            Self::Target => ", which is the instance this request selected",
            Self::ConcurrentUser => {
                ", while the hardware was generating input; Recon attributes that to the person \
                 using the Mac rather than to the target"
            }
            Self::Unexplained => {
                ", with nothing tying it to the selected target or to input from the keyboard or \
                 mouse"
            }
        }
    }
}

/// One look at the Mac. A dimension is optional because an unreadable one is a gap in the watch,
/// not a quiet desktop.
#[derive(Default)]
struct Snapshot {
    at: Duration,
    front: Option<FrontProcess>,
    front_key: Option<Option<WindowId>>,
    front_key_counters: Counters,
    front_key_watched: bool,
    space: Option<u64>,
    pointer: Option<CGPoint>,
    counters: Counters,
    /// Ordinary on-screen windows, front to back.
    windows: Vec<WindowPlace>,
    windows_read: bool,
}

impl Snapshot {
    fn unreadable(&self) -> Option<Dimension> {
        if self.front.is_none() {
            return Some(Dimension::Foreground);
        }
        if self.front_key_watched && self.front_key.is_none() {
            return Some(Dimension::KeyWindow);
        }
        if self.space.is_none() {
            return Some(Dimension::Space);
        }
        if self.pointer.is_none() {
            return Some(Dimension::Pointer);
        }
        (!self.windows_read).then_some(Dimension::WindowOrder)
    }
}

/// The front process as the window server knows it, and the process it resolves to.
///
/// The serial number answers “did the front process change”; the process ID answers “was it the
/// target”. They are separate fields because resolving the ID costs a LaunchServices round trip
/// that the guard only pays when the front process actually changed.
#[derive(Clone, Copy)]
struct FrontProcess {
    serial: u64,
    pid: Option<Pid>,
}

/// A window's place in the order the user sees.
#[derive(Clone, Copy)]
struct WindowPlace {
    id: WindowId,
    owner: Pid,
}

/// How many pointer and input events the whole session and the hardware have produced.
///
/// The pair is what separates a person from a program: every event the hardware generates is
/// counted in both, so a session count that outruns the hardware count is an event something
/// posted. It is the only honest way to tell input that was injected from input someone typed.
#[derive(Clone, Copy, Debug, Default)]
struct Counters {
    hid_pointer: u32,
    hid_input: u32,
    session_pointer: u32,
    session_input: u32,
}

impl Counters {
    fn of_this_session() -> Self {
        Self {
            hid_pointer: counted(CGEventSourceStateID::HIDSystemState, &POINTER_EVENTS),
            hid_input: counted(CGEventSourceStateID::HIDSystemState, &INPUT_EVENTS),
            session_pointer: counted(CGEventSourceStateID::CombinedSessionState, &POINTER_EVENTS),
            session_input: counted(CGEventSourceStateID::CombinedSessionState, &INPUT_EVENTS),
        }
    }
}

fn counted(state: CGEventSourceStateID, events: &[CGEventType]) -> u32 {
    events.iter().fold(0, |total, event| {
        total.wrapping_add(CGEventSource::counter_for_event_type(state, *event))
    })
}

type MainConnection = unsafe extern "C" fn() -> i32;
type ActiveSpace = unsafe extern "C" fn(i32) -> u64;
type FrontProcessOf = unsafe extern "C" fn(*mut ProcessSerialNumber) -> i32;
type ProcessPid = unsafe extern "C" fn(*const ProcessSerialNumber, *mut i32) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcessSerialNumber {
    high: u32,
    low: u32,
}

/// The interfaces this Mac offers for watching, resolved once, so that a missing one fails the
/// operation before it runs rather than halfway through it.
#[derive(Clone, Copy)]
struct Probe {
    connection: i32,
    active_space: ActiveSpace,
    front_process: FrontProcessOf,
    process_pid: Option<ProcessPid>,
}

impl Probe {
    fn of_this_mac() -> Result<Self, Failure> {
        // SAFETY: `MainConnection` is the signature of `CGSMainConnectionID`.
        let main_connection: MainConnection = unsafe { private::function(c"CGSMainConnectionID") }
            .ok_or_else(|| Dimension::Space.missing())?;
        // SAFETY: `ActiveSpace` is the signature of `CGSGetActiveSpace`.
        let active_space: ActiveSpace = unsafe { private::function(c"CGSGetActiveSpace") }
            .ok_or_else(|| Dimension::Space.missing())?;
        // SAFETY: `FrontProcessOf` is the signature of `_SLPSGetFrontProcess`.
        let front_process: FrontProcessOf = unsafe { private::function(c"_SLPSGetFrontProcess") }
            .ok_or_else(|| Dimension::Foreground.missing())?;
        // SAFETY: `ProcessPid` is the signature of `GetProcessPID`.
        let process_pid: Option<ProcessPid> = unsafe { private::function(c"GetProcessPID") };
        // SAFETY: asking for this process's own window server connection creates no window and
        // changes nothing about the session.
        let connection = unsafe { main_connection() };
        Ok(Self { connection, active_space, front_process, process_pid })
    }

    /// Fills `into` with one look at the Mac, reusing its window buffer.
    fn look(
        &self,
        keys: &Keys,
        at: Duration,
        previous: Option<&Snapshot>,
        interaction: bool,
        read_front_key: bool,
        into: &mut Snapshot,
    ) {
        into.at = at;
        into.front = self.front(previous);
        into.front_key_watched = interaction && read_front_key;
        into.front_key = if !interaction {
            None
        } else if read_front_key {
            into.front.and_then(|front| front.pid).and_then(|pid| {
                recon_accessibility_macos::window_element::focused_window_id(pid).ok()
            })
        } else {
            previous.and_then(|snapshot| snapshot.front_key)
        };
        into.space = self.space();
        into.pointer = pointer();
        into.counters = Counters::of_this_session();
        if interaction {
            into.front_key_counters = if read_front_key {
                into.counters
            } else {
                previous.map_or(Counters::default(), |snapshot| snapshot.front_key_counters)
            };
        }
        into.windows_read = windows_on_screen(keys, &mut into.windows);
    }

    fn front(&self, previous: Option<&Snapshot>) -> Option<FrontProcess> {
        let mut serial = ProcessSerialNumber { high: 0, low: 0 };
        // SAFETY: `_SLPSGetFrontProcess` writes the front process's serial number through the
        // pointer and returns a non-zero status when it cannot. It reads the window server's
        // front process; it does not set one.
        let status = unsafe { (self.front_process)(&raw mut serial) };
        if status != 0 {
            return None;
        }
        let number = (u64::from(serial.high) << 32) | u64::from(serial.low);
        let known = previous.and_then(|previous| previous.front);
        let pid = match known {
            Some(front) if front.serial == number => front.pid,
            _ => self.pid_of(&serial),
        };
        Some(FrontProcess { serial: number, pid })
    }

    fn pid_of(&self, serial: &ProcessSerialNumber) -> Option<Pid> {
        let resolve = self.process_pid?;
        let mut pid = 0;
        // SAFETY: `GetProcessPID` reads the serial number and writes the process ID it names.
        let status = unsafe { resolve(&raw const *serial, &raw mut pid) };
        if status != 0 {
            return None;
        }
        u32::try_from(pid).ok().map(Pid)
    }

    /// The Space the user is looking at. The window server answers from this process's own
    /// connection, so the question neither switches a Space nor asks for one.
    fn space(&self) -> Option<u64> {
        // SAFETY: `ActiveSpace` is the signature of `CGSGetActiveSpace`, which reads the active
        // Space of the given connection.
        Some(unsafe { (self.active_space)(self.connection) })
    }
}

/// The pointer's location, read from an event created out of nothing. Creating an event neither
/// posts nor delivers anything, and nothing moves.
fn pointer() -> Option<CGPoint> {
    CGEvent::new(None).map(|event| CGEvent::location(Some(&event)))
}

/// The ordinary windows the user can see, front to back.
///
/// Menus, the Dock, notification banners and status items live above layer zero: they come and go
/// constantly without anyone losing their place, so they are not the foreground work this
/// contract protects.
fn windows_on_screen(keys: &Keys, into: &mut Vec<WindowPlace>) -> bool {
    into.clear();
    let listed = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        kCGNullWindowID,
    );
    let Some(listed) = listed else {
        return false;
    };
    // SAFETY: the window server describes each window with a dictionary keyed by CFString.
    let listed: CFRetained<CFArray<CFDictionary<CFString, CFType>>> =
        unsafe { CFRetained::cast_unchecked(listed) };
    into.extend(listed.iter().filter_map(|entry| place_of(&entry, keys)));
    true
}

fn place_of(entry: &CFDictionary<CFString, CFType>, keys: &Keys) -> Option<WindowPlace> {
    let number = |key: &CFString| entry.get(key)?.downcast::<CFNumber>().ok();
    if number(keys.layer)?.as_i32()? != 0 {
        return None;
    }
    Some(WindowPlace {
        id: WindowId(u32::try_from(number(keys.number)?.as_i64()?).ok()?),
        owner: Pid(u32::try_from(number(keys.owner_pid)?.as_i64()?).ok()?),
    })
}

struct Keys {
    number: &'static CFString,
    owner_pid: &'static CFString,
    layer: &'static CFString,
}

impl Keys {
    fn of_the_window_server() -> Self {
        // SAFETY: the documented dictionary keys of `CGWindowListCopyWindowInfo`.
        unsafe {
            Self { number: kCGWindowNumber, owner_pid: kCGWindowOwnerPID, layer: kCGWindowLayer }
        }
    }
}

/// What the watcher was asked to watch, and the interfaces it watches through.
struct Orders {
    probe: Probe,
    /// The instance the request selected, when it names one.
    target: Option<Pid>,
    /// An interaction is watched more closely: it reads the frontmost application's key window
    /// and listens for what the workspace announces between looks.
    interaction: bool,
    setup_prompts_expected: bool,
    /// When the operation started. Every look and every announcement is dated from it, so the
    /// two accounts of the same moment carry the same clock.
    clock: Instant,
    /// The workspace's own account of activations and Space switches, for an interaction. It
    /// stops listening when the watcher drops these orders.
    notices: Option<Notices>,
}

impl Orders {
    /// Whatever the workspace has announced since the last look, timed from the start.
    fn announced(&self, into: &mut Vec<(Notice, Duration)>) {
        into.clear();
        if let Some(notices) = &self.notices {
            notices.take(self.clock, into);
        }
    }
}

/// Looks at the Mac until the operation says it is done, then takes one final look.
///
/// A guard that was dropped rather than finished leaves this thread holding the only reference
/// to the stop flag, which ends the watch as surely as asking it to stop.
fn watch(
    orders: &Orders,
    stop: &Arc<AtomicBool>,
    interrupted: &Arc<AtomicBool>,
    baseline: &std::sync::mpsc::Sender<Option<Dimension>>,
) -> Trace {
    let keys = Keys::of_the_window_server();
    let mut trace = Trace::default();
    let mut previous = Snapshot::default();
    let mut current = Snapshot::default();
    let mut keystrokes = UnmatchedKeystrokes::default();
    let mut announced = Vec::new();

    orders.probe.look(&keys, orders.clock.elapsed(), None, orders.interaction, true, &mut previous);
    trace.record(&previous);
    // Release the operation only with a readable baseline; an unreadable one fails the start.
    let _ = baseline.send(previous.unreadable());
    while !stop.load(Ordering::Acquire) && Arc::strong_count(stop) > 1 {
        thread::sleep(INTERVAL);
        orders.probe.look(
            &keys,
            orders.clock.elapsed(),
            Some(&previous),
            orders.interaction,
            false,
            &mut current,
        );
        trace.record(&current);
        let existing = trace.disturbances.len();
        // What the workspace announced happened before this look and may be the only account of
        // it, so it is recorded first and the look is compared afterwards.
        orders.announced(&mut announced);
        reported(&announced, orders.target, &previous, &current, &mut trace.disturbances);
        compare(&previous, &current, orders.target, &mut keystrokes, &mut trace.disturbances);
        if current.unreadable().is_some()
            || trace.disturbances[existing..]
                .iter()
                .any(|disturbance| disturbance.blocks_operation(orders.setup_prompts_expected))
        {
            interrupted.store(true, Ordering::Release);
        }
        mem::swap(&mut previous, &mut current);
    }
    // The operation has returned; whatever it did last is only visible now.
    orders.probe.look(
        &keys,
        orders.clock.elapsed(),
        Some(&previous),
        orders.interaction,
        true,
        &mut current,
    );
    trace.record(&current);
    orders.announced(&mut announced);
    reported(&announced, orders.target, &previous, &current, &mut trace.disturbances);
    compare(&previous, &current, orders.target, &mut keystrokes, &mut trace.disturbances);
    trace
}

/// Everything that changed between two looks at the Mac.
fn compare(
    previous: &Snapshot,
    current: &Snapshot,
    target: Option<Pid>,
    keystrokes: &mut UnmatchedKeystrokes,
    found: &mut Vec<Disturbance>,
) {
    foreground(previous, current, target, found);
    frontmost_key_window(previous, current, target, found);
    window_order(previous, current, target, found);
    space(previous, current, target, found);
    pointer_moved(previous, current, found);
    input_posted(previous, current, keystrokes, found);
}

/// What the workspace announced between two looks, as evidence.
///
/// An announcement is often the only account there is: an application activated and put back, or
/// a Space switched and switched back, between two looks leaves both of them identical. So each
/// one is recorded on its own, dated when it arrived rather than when it was noticed, and judged
/// by the same rules the looks are — the target named in an activation is the target even while
/// someone is typing, and anything else that happens while the hardware is producing input is
/// the person at the Mac.
fn reported(
    announced: &[(Notice, Duration)],
    target: Option<Pid>,
    previous: &Snapshot,
    current: &Snapshot,
    found: &mut Vec<Disturbance>,
) {
    for (notice, at) in announced {
        let disturbance = match *notice {
            Notice::Activated(pid) => {
                let became = match pid {
                    Some(pid) => format!("pid {pid}"),
                    None => "an application it could not name".to_owned(),
                };
                let disturbance = Disturbance::new(
                    InterferenceEvidence::ForegroundApplication,
                    attribute_forward(pid, target, previous, current),
                    *at,
                    &format!("the workspace announced that {became} became the active application"),
                );
                match pid {
                    Some(pid) => disturbance.about(pid, None),
                    None => disturbance,
                }
            }
            // The announcement names no process, so the Space switch points wherever the
            // frontmost application does, exactly as the look at the active Space would.
            Notice::SpaceChanged => Disturbance::new(
                InterferenceEvidence::SpaceChange,
                attribute_forward(
                    current.front.and_then(|front| front.pid),
                    target,
                    previous,
                    current,
                ),
                *at,
                "the workspace announced that the active Space changed",
            ),
        };
        note(found, disturbance);
    }
}

fn foreground(
    previous: &Snapshot,
    current: &Snapshot,
    target: Option<Pid>,
    found: &mut Vec<Disturbance>,
) {
    let (Some(before), Some(after)) = (previous.front, current.front) else {
        return;
    };
    if before.serial == after.serial {
        return;
    }
    let attribution = attribute_forward(after.pid, target, previous, current);
    let became = match after.pid {
        Some(pid) => format!("pid {pid}"),
        None => "another process".to_owned(),
    };
    let mut disturbance = Disturbance::new(
        InterferenceEvidence::ForegroundApplication,
        attribution,
        current.at,
        &format!("the frontmost application became {became}"),
    );
    if let Some(pid) = after.pid {
        disturbance = disturbance.about(pid, None);
    }
    note(found, disturbance);
}

fn frontmost_key_window(
    previous: &Snapshot,
    current: &Snapshot,
    target: Option<Pid>,
    found: &mut Vec<Disturbance>,
) {
    let (Some(before), Some(after)) = (previous.front_key, current.front_key) else {
        return;
    };
    if before == after {
        return;
    }
    let front_pid = current.front.and_then(|front| front.pid);
    let attribution = attribute_since(front_pid, target, current, previous.front_key_counters);
    let observed =
        format!("the frontmost application's key window changed from {before:?} to {after:?}");
    let mut disturbance =
        Disturbance::new(InterferenceEvidence::KeyWindow, attribution, current.at, &observed);
    if let Some(pid) = front_pid {
        disturbance = disturbance.about(pid, after);
    }
    note(found, disturbance);
}

fn window_order(
    previous: &Snapshot,
    current: &Snapshot,
    target: Option<Pid>,
    found: &mut Vec<Disturbance>,
) {
    if !previous.windows_read || !current.windows_read {
        return;
    }

    // A window that closes leaves the one behind it in front. That is a window disappearing, not
    // something coming forward, so the window that was in front has to still be on screen.
    if let (Some(before), Some(after)) = (previous.windows.first(), current.windows.first())
        && before.id != after.id
        && current.windows.iter().any(|window| window.id == before.id)
    {
        let attribution = attribute_forward(Some(after.owner), target, previous, current);
        let observed = format!(
            "window {} of pid {} came in front of window {}",
            after.id, after.owner, before.id
        );
        note(
            found,
            Disturbance::new(InterferenceEvidence::KeyWindow, attribution, current.at, &observed)
                .about(after.owner, Some(after.id)),
        );
        return;
    }

    if let Some(forward) = came_forward(previous, current) {
        let attribution = attribute_forward(Some(forward.owner), target, previous, current);
        note(
            found,
            Disturbance::new(
                InterferenceEvidence::KeyWindow,
                attribution,
                current.at,
                "the windows on screen changed order",
            ),
        );
        return;
    }

    // A target-owned window may have appeared in response to a person's input, not this read.
    if let Some(target) = target
        && let Some(opened) = current.windows.iter().find(|window| {
            window.owner == target
                && !previous.windows.iter().any(|earlier| earlier.id == window.id)
        })
    {
        note(
            found,
            Disturbance::new(
                InterferenceEvidence::KeyWindow,
                attribute(Some(target), Some(target), previous, current),
                current.at,
                &format!("window {} appeared on screen", opened.id),
            )
            .about(target, Some(opened.id)),
        );
    }
}

/// The window that came forward, if the windows both looks have in common are in a different
/// order.
///
/// The first place the two orders disagree names it: whatever sits there now was behind whatever
/// sat there before.
fn came_forward(previous: &Snapshot, current: &Snapshot) -> Option<WindowPlace> {
    let before = previous
        .windows
        .iter()
        .filter(|window| current.windows.iter().any(|other| other.id == window.id));
    let after = current
        .windows
        .iter()
        .filter(|window| previous.windows.iter().any(|other| other.id == window.id));
    before.zip(after).find(|(before, after)| before.id != after.id).map(|(_, after)| *after)
}

fn space(
    previous: &Snapshot,
    current: &Snapshot,
    target: Option<Pid>,
    found: &mut Vec<Disturbance>,
) {
    let (Some(before), Some(after)) = (previous.space, current.space) else {
        return;
    };
    if before == after {
        return;
    }
    let attribution =
        attribute(current.front.and_then(|front| front.pid), target, previous, current);
    note(
        found,
        Disturbance::new(
            InterferenceEvidence::SpaceChange,
            attribution,
            current.at,
            &format!("the active Space changed from {before} to {after}"),
        ),
    );
}

fn pointer_moved(previous: &Snapshot, current: &Snapshot, found: &mut Vec<Disturbance>) {
    let (Some(before), Some(after)) = (previous.pointer, current.pointer) else {
        return;
    };
    let posted = delta(current.counters.session_pointer, previous.counters.session_pointer);
    let by_hand = delta(current.counters.hid_pointer, previous.counters.hid_pointer);
    let moved = before.x != after.x || before.y != after.y;
    if !moved && posted == 0 {
        return;
    }

    let attribution = if posted > by_hand || by_hand == 0 {
        // Either something posted pointer events the hardware did not generate, or the pointer is
        // somewhere else with no event at all to explain it. Neither is a person's hand.
        Attribution::Unexplained
    } else {
        Attribution::ConcurrentUser
    };
    let observed = if moved {
        format!(
            "the pointer moved from ({:.0}, {:.0}) to ({:.0}, {:.0})",
            before.x, before.y, after.x, after.y
        )
    } else {
        format!("{posted} pointer events moved the pointer and left it where it started")
    };
    note(
        found,
        Disturbance::new(InterferenceEvidence::PointerLocation, attribution, current.at, &observed),
    );
}

/// How long a hardware keystroke may wait for the session to count it. While someone typed on
/// macOS 27.0.1, the session's keyboard counter trailed the hardware's by up to 50 ms, a look
/// later; this leaves room for a busier session without letting old typing excuse input posted
/// long after it.
const KEYSTROKE_MATCH_WINDOW: Duration = Duration::from_millis(250);

/// Hardware keystrokes the session has not counted yet, oldest first, with the look that saw them.
#[derive(Default)]
struct UnmatchedKeystrokes(VecDeque<(Duration, u32)>);

impl UnmatchedKeystrokes {
    /// Matches the session's new events against the hardware's recent keystrokes, oldest first,
    /// and returns how many of them no keystroke explains.
    fn unexplained(&mut self, at: Duration, by_hand: u32, posted: u32) -> u32 {
        self.0.retain(|(seen, _)| at.saturating_sub(*seen) <= KEYSTROKE_MATCH_WINDOW);
        if by_hand > 0 {
            self.0.push_back((at, by_hand));
        }
        let mut unexplained = posted;
        while unexplained > 0
            && let Some((_, waiting)) = self.0.front_mut()
        {
            let matched = unexplained.min(*waiting);
            *waiting -= matched;
            unexplained -= matched;
            if *waiting == 0 {
                self.0.pop_front();
            }
        }
        unexplained
    }
}

fn input_posted(
    previous: &Snapshot,
    current: &Snapshot,
    keystrokes: &mut UnmatchedKeystrokes,
    found: &mut Vec<Disturbance>,
) {
    let posted = delta(current.counters.session_input, previous.counters.session_input);
    let by_hand = delta(current.counters.hid_input, previous.counters.hid_input);
    let unexplained = keystrokes.unexplained(current.at, by_hand, posted);
    if unexplained == 0 {
        return;
    }
    let observed = format!(
        "{unexplained} keyboard, button or scroll events were posted into the session that the \
         hardware did not generate"
    );
    note(
        found,
        Disturbance::new(
            InterferenceEvidence::InputRouting,
            Attribution::Unexplained,
            current.at,
            &observed,
        ),
    );
}

/// Who an observed change points at.
fn attribute(
    pid: Option<Pid>,
    target: Option<Pid>,
    previous: &Snapshot,
    current: &Snapshot,
) -> Attribution {
    if used_by_hand(previous, current) {
        return Attribution::ConcurrentUser;
    }
    if pid.is_some() && pid == target {
        return Attribution::Target;
    }
    Attribution::Unexplained
}

fn attribute_since(
    pid: Option<Pid>,
    target: Option<Pid>,
    current: &Snapshot,
    counters: Counters,
) -> Attribution {
    if used_by_hand_counters(counters, current.counters) {
        return Attribution::ConcurrentUser;
    }
    if pid.is_some() && pid == target {
        return Attribution::Target;
    }
    Attribution::Unexplained
}

fn attribute_forward(
    pid: Option<Pid>,
    target: Option<Pid>,
    previous: &Snapshot,
    current: &Snapshot,
) -> Attribution {
    if pid.is_some() && pid == target {
        Attribution::Target
    } else {
        attribute(pid, target, previous, current)
    }
}

/// Whether the hardware generated any input between two looks: someone is using the Mac.
fn used_by_hand(previous: &Snapshot, current: &Snapshot) -> bool {
    used_by_hand_counters(previous.counters, current.counters)
}

fn used_by_hand_counters(previous: Counters, current: Counters) -> bool {
    delta(current.hid_pointer, previous.hid_pointer) != 0
        || delta(current.hid_input, previous.hid_input) != 0
}

fn delta(current: u32, previous: u32) -> u32 {
    current.wrapping_sub(previous)
}

/// Records a disturbance, collapsing a run of the same observation.
///
/// A dragged mouse or a person moving between applications produces the same evidence a hundred
/// times a second. The first of a run is the one that dates the disturbance; the repetitions add
/// nothing a caller can act on.
fn note(found: &mut Vec<Disturbance>, disturbance: Disturbance) {
    let repeated = found.last().is_some_and(|last| {
        last.evidence == disturbance.evidence && last.attribution == disturbance.attribution
    });
    if !repeated {
        found.push(disturbance);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;

    const TARGET: Pid = Pid(4242);
    const OTHER: Pid = Pid(99);

    fn scope() -> Scope {
        Scope { stage: InspectionStage::Read, target: Some(TARGET), setup_prompts_expected: false }
    }

    /// A quiet Mac: one application in front, three ordinary windows, nothing being typed.
    fn snapshot(at: u64) -> Snapshot {
        Snapshot {
            at: Duration::from_millis(at),
            front: Some(FrontProcess { serial: 1, pid: Some(OTHER) }),
            front_key: Some(Some(WindowId(1))),
            front_key_counters: Counters {
                hid_pointer: 10,
                hid_input: 5,
                session_pointer: 10,
                session_input: 5,
            },
            front_key_watched: false,
            space: Some(4),
            pointer: Some(CGPoint::new(100.0, 200.0)),
            counters: Counters {
                hid_pointer: 10,
                hid_input: 5,
                session_pointer: 10,
                session_input: 5,
            },
            windows: vec![place(1, OTHER), place(2, TARGET), place(3, OTHER)],
            windows_read: true,
        }
    }

    fn place(id: u32, owner: Pid) -> WindowPlace {
        WindowPlace { id: WindowId(id), owner }
    }

    fn changes(previous: &Snapshot, current: &Snapshot) -> Vec<Disturbance> {
        changes_across(&[previous, current])
    }

    /// Compares each look with the one before it, carrying what the watcher carries between them.
    fn changes_across(looks: &[&Snapshot]) -> Vec<Disturbance> {
        let mut keystrokes = UnmatchedKeystrokes::default();
        let mut found = Vec::new();
        for pair in looks.windows(2) {
            compare(pair[0], pair[1], Some(TARGET), &mut keystrokes, &mut found);
        }
        found
    }

    fn details(found: &[Disturbance]) -> Vec<&str> {
        found.iter().map(|disturbance| disturbance.detail.as_str()).collect()
    }

    fn observed(disturbances: Vec<Disturbance>) -> Observation {
        Observation { scope: scope(), trace: Some(Trace { samples: 2, disturbances, gap: None }) }
    }

    /// One interval of an interaction's watch: what the workspace announced during it, and then
    /// what the two looks themselves show, in the order the watcher records them.
    fn interval(
        announced: &[(Notice, u64)],
        previous: &Snapshot,
        current: &Snapshot,
    ) -> Vec<Disturbance> {
        let announced: Vec<(Notice, Duration)> =
            announced.iter().map(|(notice, at)| (*notice, Duration::from_millis(*at))).collect();
        let mut keystrokes = UnmatchedKeystrokes::default();
        let mut found = Vec::new();
        reported(&announced, Some(TARGET), previous, current, &mut found);
        compare(previous, current, Some(TARGET), &mut keystrokes, &mut found);
        found
    }

    /// An interaction whose caller expects a macOS setup prompt to interrupt the desktop.
    fn observed_during_setup(disturbances: Vec<Disturbance>) -> Observation {
        Observation {
            scope: Scope {
                stage: InspectionStage::Interaction,
                target: Some(TARGET),
                setup_prompts_expected: true,
            },
            trace: Some(Trace { samples: 2, disturbances, gap: None }),
        }
    }

    /// Someone at the keyboard during the interval between two looks.
    fn typed_on(look: &mut Snapshot) {
        look.counters.hid_input += 1;
        look.counters.session_input += 1;
    }

    #[test]
    fn the_target_taking_the_foreground_is_attributed_to_it() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 2, pid: Some(TARGET) });

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].evidence, InterferenceEvidence::ForegroundApplication);
        assert_eq!(found[0].attribution, Attribution::Target);
        assert_eq!(found[0].pid, Some(TARGET));
        assert!(
            found[0].detail.contains("the instance this request selected"),
            "{}",
            found[0].detail
        );
    }

    #[test]
    fn an_activation_that_is_undone_is_still_reported() {
        // Comparing only the first and last looks, as a before-and-after check does, would see
        // the same frontmost application and report nothing.
        let first = snapshot(0);
        let mut middle = snapshot(10);
        middle.front = Some(FrontProcess { serial: 2, pid: Some(TARGET) });
        let last = snapshot(20);
        assert_eq!(first.front.expect("a front process").serial, last.front.expect("one").serial);

        let found = changes_across(&[&first, &middle, &last]);

        assert_eq!(found.len(), 2);
        assert_eq!(found[0].attribution, Attribution::Target);
        assert_eq!(found[1].evidence, InterferenceEvidence::ForegroundApplication);
        assert_eq!(
            observed(found).outcome().map(|failure| failure.code),
            Some(ErrorCode::UiInterference)
        );
    }

    #[test]
    fn a_person_switching_applications_is_not_blamed_on_the_target() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 7, pid: Some(Pid(1234)) });
        after.counters.hid_input += 1;
        after.counters.session_input += 1;

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attribution, Attribution::ConcurrentUser);
        assert!(observed(found).outcome().is_none());
    }

    #[test]
    fn a_target_activation_during_hardware_input_is_still_attributed_to_the_target() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 7, pid: Some(TARGET) });
        after.counters.hid_input += 1;
        after.counters.session_input += 1;

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attribution, Attribution::Target);
        assert_eq!(
            observed(found).outcome().map(|failure| failure.code),
            Some(ErrorCode::UiInterference)
        );
    }

    #[test]
    fn a_frontmost_key_window_change_without_hardware_input_is_interference() {
        let mut before = snapshot(0);
        before.front_key = Some(Some(WindowId(42)));
        let mut after = snapshot(10);
        after.front_key = Some(Some(WindowId(99)));

        let found = changes(&before, &after);

        assert!(found.iter().any(|disturbance| {
            disturbance.evidence == InterferenceEvidence::KeyWindow
                && disturbance.attribution == Attribution::Unexplained
        }));
        assert_eq!(
            observed(found).outcome().map(|failure| failure.code),
            Some(ErrorCode::UiInterference)
        );
    }

    #[test]
    fn hardware_input_explains_a_frontmost_key_window_change() {
        let mut before = snapshot(0);
        before.front_key = Some(Some(WindowId(42)));
        let mut after = snapshot(10);
        after.front_key = Some(Some(WindowId(99)));
        after.counters.hid_input += 1;
        after.counters.session_input += 1;

        let found = changes(&before, &after);

        assert!(found.iter().any(|disturbance| {
            disturbance.evidence == InterferenceEvidence::KeyWindow
                && disturbance.attribution == Attribution::ConcurrentUser
        }));
        assert!(observed(found).outcome().is_none());
    }

    #[test]
    fn hardware_input_since_the_key_window_read_explains_a_quiet_final_interval() {
        let mut before = snapshot(0);
        before.front_key = Some(Some(WindowId(42)));
        before.counters.hid_input += 1;
        before.counters.session_input += 1;
        let mut after = snapshot(10);
        after.front_key = Some(Some(WindowId(99)));
        after.counters = before.counters;

        let found = changes(&before, &after);

        assert!(found.iter().any(|disturbance| {
            disturbance.evidence == InterferenceEvidence::KeyWindow
                && disturbance.attribution == Attribution::ConcurrentUser
        }));
        assert!(observed(found).outcome().is_none());
    }

    #[test]
    fn an_unexplained_foreground_change_fails_the_operation() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 7, pid: Some(Pid(1234)) });

        let found = changes(&before, &after);

        assert_eq!(found[0].attribution, Attribution::Unexplained);
        let failure = observed(found).outcome().expect("an unexplained change fails");
        assert_eq!(failure.code, ErrorCode::UiInterference);
        assert!(!failure.retryable);
        assert!(failure.message.contains("nothing tying it"), "{}", failure.message);
    }

    #[test]
    fn the_failure_names_the_stage_the_evidence_and_the_process() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 2, pid: Some(TARGET) });

        let failure = observed(changes(&before, &after)).outcome().expect("a failure");

        let details = failure.details.as_ref().expect("details");
        assert_eq!(details["stage"], "read");
        assert_eq!(details["evidenceSource"], "foreground-application");
        assert_eq!(details["pid"], TARGET.0);
        assert_eq!(details["targetExecutionMayContinue"], Value::Bool(true));
        assert_eq!(details["disturbance"], failure.message);
    }

    #[test]
    fn a_window_closing_is_not_a_raise() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.windows = vec![place(2, TARGET), place(3, OTHER)];

        assert!(changes(&before, &after).is_empty());
    }

    #[test]
    fn a_window_coming_forward_is_reported_with_its_owner() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.windows = vec![place(2, TARGET), place(1, OTHER), place(3, OTHER)];

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].evidence, InterferenceEvidence::KeyWindow);
        assert_eq!(found[0].attribution, Attribution::Target);
        assert_eq!(found[0].window, Some(WindowId(2)));
    }

    #[test]
    fn a_reordering_behind_the_front_window_is_still_a_disturbance() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.windows = vec![place(1, OTHER), place(3, OTHER), place(2, TARGET)];

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].evidence, InterferenceEvidence::KeyWindow);
    }

    /// A target window rising past a window it was behind is the target's doing even while the
    /// person is typing: the front window never moves, so only the order behind it shows it.
    #[test]
    fn a_target_window_rising_behind_an_unchanged_front_window_is_the_targets_doing() {
        let mut before = snapshot(0);
        before.windows = vec![place(1, OTHER), place(3, OTHER), place(2, TARGET)];
        let mut after = snapshot(10);
        after.counters.hid_input += 1;
        after.counters.session_input += 1;

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].evidence, InterferenceEvidence::KeyWindow);
        assert_eq!(found[0].attribution, Attribution::Target);
        assert_eq!(
            observed(found).outcome().map(|failure| failure.code),
            Some(ErrorCode::UiInterference)
        );
    }

    #[test]
    fn a_window_the_target_puts_on_screen_is_reported_as_its_own() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.windows.push(place(9, TARGET));

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attribution, Attribution::Target);
        assert_eq!(found[0].window, Some(WindowId(9)));
    }

    #[test]
    fn a_user_opening_a_window_in_the_target_is_not_blamed_on_a_read() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.windows.push(place(9, TARGET));
        after.counters.hid_input += 1;
        after.counters.session_input += 1;

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attribution, Attribution::ConcurrentUser);
        assert!(observed(found).outcome().is_none());
    }

    #[test]
    fn a_window_another_application_opens_is_not_the_targets_doing() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.windows.push(place(9, OTHER));

        assert!(changes(&before, &after).is_empty());
    }

    #[test]
    fn moving_the_pointer_and_putting_it_back_is_still_reported() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.counters.session_pointer += 4;

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].evidence, InterferenceEvidence::PointerLocation);
        assert_eq!(found[0].attribution, Attribution::Unexplained);
        assert!(found[0].detail.contains("left it where it started"), "{}", found[0].detail);
    }

    #[test]
    fn a_pointer_warp_with_no_event_at_all_is_reported() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.pointer = Some(CGPoint::new(640.0, 480.0));

        let found = changes(&before, &after);

        assert_eq!(found[0].evidence, InterferenceEvidence::PointerLocation);
        assert_eq!(found[0].attribution, Attribution::Unexplained);
    }

    #[test]
    fn a_person_moving_the_mouse_is_not_interference() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.pointer = Some(CGPoint::new(640.0, 480.0));
        after.counters.hid_pointer += 3;
        after.counters.session_pointer += 3;

        let found = changes(&before, &after);

        assert_eq!(found[0].attribution, Attribution::ConcurrentUser);
        assert!(observed(found).outcome().is_none());
    }

    #[test]
    fn posted_keystrokes_are_reported_as_diverted_input() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.counters.session_input += 2;

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].evidence, InterferenceEvidence::InputRouting);
        assert!(found[0].detail.contains("2 keyboard"), "{}", found[0].detail);
    }

    #[test]
    fn typing_during_an_operation_is_not_diverted_input() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.counters.hid_input += 2;
        after.counters.session_input += 2;

        assert!(changes(&before, &after).is_empty());
    }

    /// Measured on macOS 27.0.1 while someone typed: the hardware counts a keystroke in one look
    /// and the session counts the same keystroke in the next, up to 50 ms later.
    #[test]
    fn a_keystroke_the_session_counts_a_look_late_is_not_diverted_input() {
        let first = snapshot(0);
        let mut second = snapshot(10);
        second.counters.hid_input += 1;
        let mut third = snapshot(20);
        third.counters.hid_input += 1;
        third.counters.session_input += 1;

        let found = changes_across(&[&first, &second, &third]);

        assert!(found.is_empty(), "{:?}", details(&found));
    }

    #[test]
    fn a_late_keystroke_does_not_excuse_more_posted_input_than_it_explains() {
        let first = snapshot(0);
        let mut second = snapshot(10);
        second.counters.hid_input += 1;
        let mut third = snapshot(20);
        third.counters.hid_input += 1;
        third.counters.session_input += 3;

        let found = changes_across(&[&first, &second, &third]);

        assert_eq!(details(&found).len(), 1, "{:?}", details(&found));
        assert_eq!(found[0].evidence, InterferenceEvidence::InputRouting);
        assert!(found[0].detail.contains("2 keyboard"), "{}", found[0].detail);
    }

    /// A keystroke the session never counted, such as one the system kept for itself, stops
    /// explaining posted input once far more time has passed than the session ever lags.
    #[test]
    fn a_keystroke_long_past_does_not_excuse_posted_input() {
        let first = snapshot(0);
        let mut second = snapshot(10);
        second.counters.hid_input += 1;
        let mut third = snapshot(1_000);
        third.counters.hid_input += 1;
        third.counters.session_input += 1;

        let found = changes_across(&[&first, &second, &third]);

        assert_eq!(details(&found).len(), 1, "{:?}", details(&found));
        assert!(found[0].detail.contains("1 keyboard"), "{}", found[0].detail);
    }

    #[test]
    fn a_space_change_is_reported() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.space = Some(29);

        let found = changes(&before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].evidence, InterferenceEvidence::SpaceChange);
        assert_eq!(found[0].attribution, Attribution::Unexplained);
    }

    #[test]
    fn the_most_pointed_evidence_is_the_one_reported() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.space = Some(29);
        after.front = Some(FrontProcess { serial: 2, pid: Some(TARGET) });

        let failure = observed(changes(&before, &after)).outcome().expect("a failure");

        let details = failure.details.as_ref().expect("details");
        assert_eq!(details["evidenceSource"], "foreground-application");
    }

    #[test]
    fn repeated_evidence_is_recorded_once() {
        let mut keystrokes = UnmatchedKeystrokes::default();
        let mut found = Vec::new();
        for step in 1..4 {
            let before = snapshot(u64::from(step) * 10);
            let mut after = snapshot(u64::from(step) * 10 + 10);
            after.pointer = Some(CGPoint::new(f64::from(step), 0.0));
            after.counters.hid_pointer += step;
            after.counters.session_pointer += step;
            compare(&before, &after, Some(TARGET), &mut keystrokes, &mut found);
        }

        assert_eq!(found.len(), 1);
    }

    #[test]
    fn a_setup_prompt_that_was_expected_does_not_fail_the_operation() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 8, pid: Some(Pid(501)) });
        let observation = Observation {
            scope: Scope {
                stage: InspectionStage::Attach,
                target: Some(TARGET),
                setup_prompts_expected: true,
            },
            trace: Some(Trace { samples: 2, disturbances: changes(&before, &after), gap: None }),
        };

        assert!(observation.outcome().is_none());
    }

    #[test]
    fn an_expected_prompt_does_not_excuse_the_target_taking_the_foreground() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        // The target itself comes forward: that is target activation, not the authorization dialog.
        after.front = Some(FrontProcess { serial: 8, pid: Some(TARGET) });
        let observation = Observation {
            scope: Scope {
                stage: InspectionStage::Attach,
                target: Some(TARGET),
                setup_prompts_expected: true,
            },
            trace: Some(Trace { samples: 2, disturbances: changes(&before, &after), gap: None }),
        };

        let failure = observation.outcome().expect("target activation is still reported");
        assert_eq!(failure.code, ErrorCode::UiInterference);
    }

    fn permission_denied() -> Failure {
        Failure::new(
            ErrorCode::PermissionDenied,
            "missing permission: Screen Recording",
            Value::Null,
        )
    }

    /// A missing grant is requested and then reported; macOS's request dialog may come forward
    /// before the guard's last look. That dialog is the setup prompt the no-focus contract allows.
    #[test]
    fn a_permission_request_reports_permission_denied_rather_than_its_dialog() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 8, pid: Some(Pid(501)) });

        let failure = observed(changes(&before, &after))
            .resolve::<()>(Err(permission_denied()))
            .expect_err("the request still fails");

        assert_eq!(failure.code, ErrorCode::PermissionDenied, "{}", failure.message);
    }

    #[test]
    fn a_permission_request_does_not_excuse_the_target_taking_the_foreground() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 8, pid: Some(TARGET) });

        let failure = observed(changes(&before, &after))
            .resolve::<()>(Err(permission_denied()))
            .expect_err("the request still fails");

        assert_eq!(failure.code, ErrorCode::UiInterference, "{}", failure.message);
    }

    #[test]
    fn an_expected_prompt_does_not_excuse_a_space_switch() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 8, pid: Some(Pid(501)) });
        after.space = Some(5);
        let observation = Observation {
            scope: Scope {
                stage: InspectionStage::Attach,
                target: Some(TARGET),
                setup_prompts_expected: true,
            },
            trace: Some(Trace { samples: 2, disturbances: changes(&before, &after), gap: None }),
        };
        let failure = observation.outcome().expect("setup does not excuse a Space switch");
        assert_eq!(failure.code, ErrorCode::UiInterference);
        assert_eq!(failure.details.expect("details")["evidenceSource"], "space-change");
    }

    #[test]
    fn losing_sight_of_a_dimension_fails_closed() {
        let observation = Observation {
            scope: scope(),
            trace: Some(Trace {
                samples: 2,
                disturbances: Vec::new(),
                gap: Some(Gap { dimension: Dimension::Space, at: Duration::from_millis(30) }),
            }),
        };

        let failure = observation.outcome().expect("an unwatched dimension fails closed");
        assert_eq!(failure.code, ErrorCode::InterfaceUnavailable);
        let details = failure.details.as_ref().expect("details");
        assert_eq!(details["interface"], "no-focus-space");
        assert_eq!(details["symbol"], "CGSGetActiveSpace");
    }

    #[test]
    fn an_unreadable_sample_is_a_gap() {
        let mut trace = Trace::default();
        let mut blind = snapshot(10);
        blind.front = None;

        trace.record(&snapshot(0));
        trace.record(&blind);

        assert_eq!(trace.samples, 2);
        assert_eq!(trace.gap.expect("a gap").dimension, Dimension::Foreground);
    }

    #[test]
    fn a_watcher_that_stops_cannot_report_a_quiet_desktop() {
        let failure = Observation { scope: scope(), trace: None }
            .outcome()
            .expect("a lost watcher fails closed");

        assert_eq!(failure.code, ErrorCode::Internal);
    }

    #[test]
    fn watching_this_mac_produces_a_continuous_trace() {
        let guard = match Guard::start(scope()) {
            Ok(guard) => guard,
            // A Mac without these interfaces is the case the guard fails closed on.
            Err(failure) => {
                assert_eq!(failure.code, ErrorCode::InterfaceUnavailable);
                return;
            }
        };

        thread::sleep(Duration::from_millis(200));
        let observation = guard.finish();

        let trace = observation.trace.as_ref().expect("the watcher finished");
        // More than two looks is the whole point: a before-and-after check has exactly two, and
        // a machine under load stretches the interval without making the watch a pair.
        assert!(trace.samples > 2, "{} samples in 200 ms", trace.samples);
        if trace.gap.is_some() {
            // A dimension this Mac cannot show must stop the operation, not be assumed quiet.
            let failure = observation.outcome().expect("a gap fails closed");
            assert_eq!(failure.code, ErrorCode::InterfaceUnavailable);
        }
    }

    #[test]
    fn a_failed_operation_reports_the_observed_interference_and_its_original_cause() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 7, pid: Some(TARGET) });
        let observation = observed(changes(&before, &after));
        let original: Result<(), Failure> =
            Err(Failure::new(ErrorCode::NotRunning, "getter failed", Value::Null));

        let failure = observation.resolve(original).expect_err("the operation disturbed focus");
        assert_eq!(failure.code, ErrorCode::UiInterference);
        assert!(failure.message.contains("getter failed"), "{}", failure.message);
        assert_eq!(failure.details.expect("details")["evidenceSource"], "foreground-application");
    }

    #[test]
    fn an_activation_the_workspace_announces_for_the_target_is_attributed_to_it() {
        let before = snapshot(0);
        let after = snapshot(10);

        let found = interval(&[(Notice::Activated(Some(TARGET)), 4)], &before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].evidence, InterferenceEvidence::ForegroundApplication);
        assert_eq!(found[0].attribution, Attribution::Target);
        assert_eq!(found[0].pid, Some(TARGET));
        // The announcement dates the disturbance: it is when the desktop changed, not when the
        // guard next looked.
        assert_eq!(found[0].at, Duration::from_millis(4));
        assert_eq!(
            observed(found).outcome().map(|failure| failure.code),
            Some(ErrorCode::UiInterference)
        );
    }

    /// The case the looks cannot see: the activation is over before the next one.
    #[test]
    fn an_activation_the_workspace_announced_and_undone_still_fails_the_operation() {
        let before = snapshot(0);
        let after = snapshot(10);
        assert!(changes(&before, &after).is_empty(), "the looks themselves show a quiet desktop");

        let found = interval(
            &[(Notice::Activated(Some(TARGET)), 2), (Notice::Activated(Some(OTHER)), 6)],
            &before,
            &after,
        );

        assert_eq!(details(&found).len(), 2, "{:?}", details(&found));
        assert_eq!(found[0].attribution, Attribution::Target);
        assert_eq!(found[1].attribution, Attribution::Unexplained);
        let failure = observed(found).outcome().expect("an activation that was undone still fails");
        assert_eq!(failure.code, ErrorCode::UiInterference);
        let details = failure.details.as_ref().expect("details");
        assert_eq!(details["evidenceSource"], "foreground-application");
        assert_eq!(details["pid"], TARGET.0);
        assert_eq!(details["targetExecutionMayContinue"], Value::Bool(true));
    }

    #[test]
    fn an_announced_target_activation_during_hardware_input_is_still_the_targets() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        typed_on(&mut after);

        let found = interval(&[(Notice::Activated(Some(TARGET)), 5)], &before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attribution, Attribution::Target);
        // What blocks the operation is also what interrupts the watch that is still running.
        assert!(found[0].blocks_operation(false));
    }

    #[test]
    fn an_announced_activation_of_another_application_during_input_is_the_persons() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        typed_on(&mut after);

        let found = interval(&[(Notice::Activated(Some(Pid(1234))), 5)], &before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attribution, Attribution::ConcurrentUser);
        assert!(!found[0].blocks_operation(false));
        assert!(observed(found).outcome().is_none());
    }

    #[test]
    fn an_announced_activation_nothing_explains_fails_without_blaming_the_target() {
        let before = snapshot(0);
        let after = snapshot(10);

        let found = interval(&[(Notice::Activated(Some(Pid(1234))), 5)], &before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attribution, Attribution::Unexplained);
        let failure = observed(found).outcome().expect("an unexplained activation fails");
        assert_eq!(failure.code, ErrorCode::UiInterference);
        assert!(failure.message.contains("nothing tying it"), "{}", failure.message);
    }

    #[test]
    fn an_announced_activation_the_workspace_cannot_name_is_not_the_targets() {
        let before = snapshot(0);
        let after = snapshot(10);

        let found = interval(&[(Notice::Activated(None), 5)], &before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attribution, Attribution::Unexplained);
        assert_eq!(found[0].pid, None);
        assert!(found[0].detail.contains("could not name"), "{}", found[0].detail);
    }

    /// A Space that is switched and switched back leaves both looks on the same Space.
    #[test]
    fn a_space_switch_the_workspace_announced_and_undone_still_fails_the_operation() {
        let before = snapshot(0);
        let after = snapshot(10);
        assert!(changes(&before, &after).is_empty(), "the looks themselves show a quiet desktop");

        let found =
            interval(&[(Notice::SpaceChanged, 3), (Notice::SpaceChanged, 7)], &before, &after);

        // The switch and the switch back are one run of the same evidence; the first of them
        // dates it.
        assert_eq!(details(&found).len(), 1, "{:?}", details(&found));
        assert_eq!(found[0].evidence, InterferenceEvidence::SpaceChange);
        assert_eq!(found[0].at, Duration::from_millis(3));
        let failure = observed(found).outcome().expect("a Space switch that was undone fails");
        assert_eq!(failure.details.expect("details")["evidenceSource"], "space-change");
    }

    #[test]
    fn a_space_switch_announced_while_someone_is_typing_is_not_interference() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        typed_on(&mut after);

        let found = interval(&[(Notice::SpaceChanged, 4)], &before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attribution, Attribution::ConcurrentUser);
        assert!(observed(found).outcome().is_none());
    }

    /// The announcement names no process, so the Space it switched to points where the frontmost
    /// application does — the same reading the look at the active Space makes.
    #[test]
    fn a_space_switch_announced_while_the_target_is_in_front_is_its_doing() {
        let front = Some(FrontProcess { serial: 2, pid: Some(TARGET) });
        let mut before = snapshot(0);
        before.front = front;
        let mut after = snapshot(10);
        after.front = front;
        typed_on(&mut after);

        let found = interval(&[(Notice::SpaceChanged, 4)], &before, &after);

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].attribution, Attribution::Target);
    }

    #[test]
    fn an_expected_setup_prompt_does_not_excuse_an_announced_space_switch() {
        let before = snapshot(0);
        let after = snapshot(10);

        let observation =
            observed_during_setup(interval(&[(Notice::SpaceChanged, 4)], &before, &after));

        let failure = observation.outcome().expect("setup does not excuse a Space switch");
        assert_eq!(failure.code, ErrorCode::UiInterference);
        assert_eq!(failure.details.expect("details")["evidenceSource"], "space-change");
    }

    #[test]
    fn an_expected_setup_prompt_excuses_the_activation_it_announces() {
        let before = snapshot(0);
        let after = snapshot(10);

        let observation = observed_during_setup(interval(
            &[(Notice::Activated(Some(Pid(501))), 4)],
            &before,
            &after,
        ));

        assert!(observation.outcome().is_none());
    }

    #[test]
    fn an_expected_setup_prompt_does_not_excuse_an_announced_target_activation() {
        let before = snapshot(0);
        let after = snapshot(10);

        let observation = observed_during_setup(interval(
            &[(Notice::Activated(Some(TARGET)), 4)],
            &before,
            &after,
        ));

        let failure = observation.outcome().expect("target activation is still reported");
        assert_eq!(failure.code, ErrorCode::UiInterference);
    }

    /// An activation that lasted long enough to be seen is announced and looked at. It is one
    /// disturbance, dated when the workspace said it happened.
    #[test]
    fn an_activation_both_heard_and_seen_is_reported_once() {
        let before = snapshot(0);
        let mut after = snapshot(10);
        after.front = Some(FrontProcess { serial: 2, pid: Some(TARGET) });

        let found = interval(&[(Notice::Activated(Some(TARGET)), 4)], &before, &after);

        assert_eq!(details(&found).len(), 1, "{:?}", details(&found));
        assert_eq!(found[0].at, Duration::from_millis(4));
    }

    #[test]
    fn an_interaction_listens_for_what_a_look_would_miss() {
        let scope = Scope {
            stage: InspectionStage::Interaction,
            target: Some(TARGET),
            setup_prompts_expected: false,
        };
        let guard = match Guard::start(scope) {
            Ok(guard) => guard,
            // A Mac that cannot be watched or listened to is the case the guard fails closed on,
            // before the operation sends anything.
            Err(failure) => {
                assert_eq!(failure.code, ErrorCode::InterfaceUnavailable);
                return;
            }
        };

        thread::sleep(Duration::from_millis(100));
        let observation = guard.finish();

        // Finishing stops the observers with the watcher; the trace it returns is the whole of
        // what the guard heard and saw.
        let trace = observation.trace.as_ref().expect("the watcher finished");
        assert!(trace.samples > 2, "{} samples in 100 ms", trace.samples);
    }
}
