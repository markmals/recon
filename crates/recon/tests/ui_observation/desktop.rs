//! A continuous watch over this Mac, kept from outside Recon while a `ui` command runs.
//!
//! The no-focus contract is about what happens *during* an operation, so this watch samples
//! rather than comparing the desktop before and after: an activation that is undone, or a
//! pointer that is moved and put back, leaves no trace in a before-and-after pair. The watch is
//! made through the same interfaces anyone could use, so a failure says that Recon disturbed
//! the Mac rather than that a test failed to notice it did.
//!
//! The watch only reads. It never activates an application, moves the pointer, or posts an
//! event.

use std::ffi::CStr;
use std::mem::{size_of, transmute_copy};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_core_graphics::{
    CGEvent, CGEventSource, CGEventSourceStateID, CGEventType, CGWindowListCopyWindowInfo,
    CGWindowListOption, kCGNullWindowID, kCGWindowLayer, kCGWindowNumber, kCGWindowOwnerPID,
};

/// How often the watch looks at the Mac, matching the guard inside Recon.
pub(crate) const INTERVAL: Duration = Duration::from_millis(10);

/// What the watch saw while it was running.
#[derive(Debug, Default)]
pub(crate) struct Seen {
    pub(crate) samples: usize,
    /// The longest a single look was away from the previous one.
    pub(crate) longest_interval: Duration,
    /// Every process that became frontmost, in order.
    pub(crate) foreground: Vec<u32>,
    /// Ordinary windows coming forward or changing places.
    pub(crate) window_order: usize,
    pub(crate) space_changes: usize,
    pub(crate) pointer_moves: usize,
    /// Keyboard, button and scroll events that reached the session without the hardware
    /// generating them: input something posted.
    pub(crate) posted_input: u32,
    /// Key and button events the hardware generated: the person at the Mac.
    pub(crate) hardware_input: u32,
    /// A dimension of the contract this Mac would not report.
    pub(crate) unreadable: Vec<&'static str>,
}

impl Seen {
    pub(crate) fn undisturbed(&self) -> bool {
        self.foreground.is_empty()
            && self.window_order == 0
            && self.space_changes == 0
            && self.pointer_moves == 0
            && self.posted_input == 0
    }
}

/// A running watch over the Mac, in its own thread.
pub(crate) struct Watch {
    stop: Arc<AtomicBool>,
    watcher: JoinHandle<Seen>,
}

impl Watch {
    pub(crate) fn start() -> Self {
        let probe = Probe::of_this_mac();
        let stop = Arc::new(AtomicBool::new(false));
        let watching = Arc::clone(&stop);
        let watcher = thread::spawn(move || probe.watch(&watching));
        Self { stop, watcher }
    }

    pub(crate) fn stop(self) -> Seen {
        self.stop.store(true, Ordering::Release);
        self.watcher.join().expect("the watch finished")
    }
}

type MainConnection = unsafe extern "C" fn() -> i32;
type ActiveSpace = unsafe extern "C" fn(i32) -> u64;
type FrontProcess = unsafe extern "C" fn(*mut ProcessSerialNumber) -> i32;
type ProcessPid = unsafe extern "C" fn(*const ProcessSerialNumber, *mut i32) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
struct ProcessSerialNumber {
    high: u32,
    low: u32,
}

#[derive(Clone, Copy)]
struct Probe {
    connection: Option<i32>,
    active_space: Option<ActiveSpace>,
    front_process: Option<FrontProcess>,
    process_pid: Option<ProcessPid>,
}

#[derive(Default)]
struct Sample {
    front: Option<u64>,
    front_pid: Option<u32>,
    space: Option<u64>,
    pointer: Option<(f64, f64)>,
    windows: Vec<(u32, u32)>,
    windows_read: bool,
    hid_pointer: u32,
    hid_input: u32,
    session_pointer: u32,
    session_input: u32,
}

impl Probe {
    fn of_this_mac() -> Self {
        // SAFETY: each type is the signature of the symbol it is resolved with. The window server
        // interfaces are read-only queries.
        unsafe {
            let main: Option<MainConnection> = symbol(c"CGSMainConnectionID");
            Self {
                connection: main.map(|main| main()),
                active_space: symbol(c"CGSGetActiveSpace"),
                front_process: symbol(c"_SLPSGetFrontProcess"),
                process_pid: symbol(c"GetProcessPID"),
            }
        }
    }

    fn watch(self, stop: &AtomicBool) -> Seen {
        let keys = Keys::of_the_window_server();
        let clock = Instant::now();
        let mut seen = Seen::default();
        let mut previous = self.look(&keys, None);
        let mut last = clock.elapsed();
        seen.samples = 1;
        self.unreadable(&previous, &mut seen);

        loop {
            let stopping = stop.load(Ordering::Acquire);
            let current = self.look(&keys, Some(&previous));
            let at = clock.elapsed();
            seen.samples += 1;
            seen.longest_interval = seen.longest_interval.max(at - last);
            last = at;
            self.unreadable(&current, &mut seen);
            compare(&previous, &current, &mut seen);
            previous = current;
            if stopping {
                return seen;
            }
            thread::sleep(INTERVAL);
        }
    }

    fn unreadable(self, sample: &Sample, seen: &mut Seen) {
        for (readable, dimension) in [
            (sample.front.is_some(), "the frontmost application"),
            (sample.space.is_some(), "the active Space"),
            (sample.pointer.is_some(), "the pointer"),
            (sample.windows_read, "the order of the windows on screen"),
        ] {
            if !readable && !seen.unreadable.contains(&dimension) {
                seen.unreadable.push(dimension);
            }
        }
    }

    fn look(self, keys: &Keys, previous: Option<&Sample>) -> Sample {
        let mut sample = Sample {
            space: self.space(),
            pointer: CGEvent::new(None)
                .map(|event| CGEvent::location(Some(&event)))
                .map(|point| (point.x, point.y)),
            hid_pointer: counted(CGEventSourceStateID::HIDSystemState, CGEventType::MouseMoved),
            hid_input: counted(CGEventSourceStateID::HIDSystemState, CGEventType::KeyDown),
            session_pointer: counted(
                CGEventSourceStateID::CombinedSessionState,
                CGEventType::MouseMoved,
            ),
            session_input: counted(
                CGEventSourceStateID::CombinedSessionState,
                CGEventType::KeyDown,
            ),
            ..Sample::default()
        };
        (sample.front, sample.front_pid) = self.front(previous);
        sample.windows_read = windows_on_screen(keys, &mut sample.windows);
        sample
    }

    fn front(self, previous: Option<&Sample>) -> (Option<u64>, Option<u32>) {
        let Some(ask) = self.front_process else {
            return (None, None);
        };
        let mut serial = ProcessSerialNumber { high: 0, low: 0 };
        // SAFETY: `_SLPSGetFrontProcess` writes the front process's serial number and returns a
        // non-zero status when it cannot. It reads the front process; it does not set one.
        let status = unsafe { ask(&raw mut serial) };
        if status != 0 {
            return (None, None);
        }
        let number = (u64::from(serial.high) << 32) | u64::from(serial.low);
        match previous {
            Some(previous) if previous.front == Some(number) => (Some(number), previous.front_pid),
            _ => (Some(number), self.pid_of(&serial)),
        }
    }

    fn pid_of(self, serial: &ProcessSerialNumber) -> Option<u32> {
        let resolve = self.process_pid?;
        let mut pid = 0;
        // SAFETY: `GetProcessPID` reads the serial number and writes the process ID it names.
        let status = unsafe { resolve(&raw const *serial, &raw mut pid) };
        (status == 0).then_some(())?;
        u32::try_from(pid).ok()
    }

    fn space(self) -> Option<u64> {
        let (connection, ask) = (self.connection?, self.active_space?);
        // SAFETY: `ActiveSpace` is the signature of `CGSGetActiveSpace`, which reads the active
        // Space of the given connection.
        Some(unsafe { ask(connection) })
    }
}

fn compare(previous: &Sample, current: &Sample, seen: &mut Seen) {
    if let (Some(before), Some(after)) = (previous.front, current.front)
        && before != after
    {
        seen.foreground.push(current.front_pid.unwrap_or_default());
    }
    if let (Some(before), Some(after)) = (previous.space, current.space)
        && before != after
    {
        seen.space_changes += 1;
    }
    // A pointer the hardware moved is the person at the Mac, not the command under test.
    if current.hid_pointer.wrapping_sub(previous.hid_pointer) == 0
        && let (Some(before), Some(after)) = (previous.pointer, current.pointer)
        && (before != after || current.session_pointer.wrapping_sub(previous.session_pointer) != 0)
    {
        seen.pointer_moves += 1;
    }
    let hardware = current.hid_input.wrapping_sub(previous.hid_input);
    seen.hardware_input += hardware;
    seen.posted_input +=
        current.session_input.wrapping_sub(previous.session_input).saturating_sub(hardware);
    if previous.windows_read && current.windows_read && disordered(previous, current) {
        seen.window_order += 1;
    }
}

/// Whether the windows both looks have in common are in a different order.
fn disordered(previous: &Sample, current: &Sample) -> bool {
    let before = previous
        .windows
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| current.windows.iter().any(|(other, _)| other == id));
    let after = current
        .windows
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| previous.windows.iter().any(|(other, _)| other == id));
    !before.eq(after)
}

fn counted(state: CGEventSourceStateID, event: CGEventType) -> u32 {
    CGEventSource::counter_for_event_type(state, event)
}

/// The ordinary windows the user can see, front to back. Menus, the Dock and notification
/// banners live above layer zero and come and go without anyone losing their place.
fn windows_on_screen(keys: &Keys, into: &mut Vec<(u32, u32)>) -> bool {
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
    into.extend(listed.iter().filter_map(|entry| {
        let number = |key: &CFString| entry.get(key)?.downcast::<CFNumber>().ok();
        if number(keys.layer)?.as_i32()? != 0 {
            return None;
        }
        Some((
            u32::try_from(number(keys.number)?.as_i64()?).ok()?,
            u32::try_from(number(keys.owner_pid)?.as_i64()?).ok()?,
        ))
    }));
    true
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

/// Resolves a macOS function by name without linking it.
///
/// # Safety
///
/// `F` must be an `unsafe extern "C" fn` type whose signature matches the named function.
pub(crate) unsafe fn symbol<F: Copy>(name: &CStr) -> Option<F> {
    const { assert!(size_of::<F>() == size_of::<*mut std::ffi::c_void>(), "not a function") };
    // SAFETY: `RTLD_DEFAULT` searches the already-loaded images for a null-terminated name.
    let address = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    if address.is_null() {
        return None;
    }
    // SAFETY: `F` is pointer sized, and the caller guarantees it is the function's signature.
    Some(unsafe { transmute_copy(&address) })
}
