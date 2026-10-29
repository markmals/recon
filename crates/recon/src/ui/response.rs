//! The JSON object each `recon ui` command prints on success.
//!
//! The native models here describe observations, not objects Recon owns: every record says which
//! live object it came from, which coordinate space its numbers are in, and what the bounded walk
//! did not reach. A field Recon could not read carries an explicit status, so an unavailable
//! observation never arrives as an empty value.
//!
//! These models publish the shape the helper and the host actually print. Where a group's members
//! depend on the kind of object observed, the model names the members it knows and stays open for
//! the rest rather than promising a closed set it cannot keep.

use std::path::Path;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Map, Value};

use crate::application::Pid;
use crate::ui::request::{AccessMode, AccessProfile, FieldGroup, NodeHandle};
use crate::ui::tree::Node;
use crate::ui::window::{Bounds, Window, WindowId};

const CAPTURE_METHOD: &str = "skylight-hardware-capture";

/// The bundle and the running instance a result describes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObservedApplication<'a> {
    pub path: &'a Path,
    pub pid: Pid,
}

/// What `recon ui windows` prints.
///
/// Without a session this is exactly the window server inventory it has always been. A session
/// adds the instance's native windows as a separate inventory; neither list is derived from the
/// other, and a native window without a proven window server identity keeps its native handle
/// rather than borrowing an id.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowsResponse<'a> {
    pub application: ObservedApplication<'a>,
    pub windows: Vec<Window>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_windows: Option<Vec<NativeRecord>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<NativeCoverage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation: Option<NativeObservation>,
}

impl<'a> WindowsResponse<'a> {
    /// The unbound inventory: the window server windows of one instance.
    pub fn new(application: ObservedApplication<'a>, windows: Vec<Window>) -> Self {
        Self {
            application,
            windows,
            session: None,
            native_windows: None,
            coverage: None,
            observation: None,
        }
    }

    /// The same inventory accompanied by the session's native windows.
    pub fn with_native_windows(
        mut self,
        session: SessionSummary,
        native_windows: Vec<NativeRecord>,
        coverage: NativeCoverage,
        observation: NativeObservation,
    ) -> Self {
        self.session = Some(session);
        self.native_windows = Some(native_windows);
        self.coverage = Some(coverage);
        self.observation = Some(observation);
        self
    }
}

/// What `recon ui ax tree` prints. `window_id` is the `--window` the command was given, if any.
/// `truncated` reports what Recon could not finish reading and `narrowed` what the caller's
/// options told it to leave out; both are always present, so a stored result answers "is this
/// the whole tree?" on its own.
///
/// A `session` records only that the observation was bound to that session's instance: this tree
/// is external accessibility evidence either way.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TreeResponse<'a> {
    pub application: ObservedApplication<'a>,
    pub window_id: Option<WindowId>,
    pub truncated: bool,
    pub narrowed: bool,
    pub root: Node,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionSummary>,
}

impl<'a> TreeResponse<'a> {
    pub fn new(
        application: ObservedApplication<'a>,
        window_id: Option<WindowId>,
        truncated: bool,
        narrowed: bool,
        root: Node,
    ) -> Self {
        Self { application, window_id, truncated, narrowed, root, session: None }
    }

    pub fn with_session(mut self, session: SessionSummary) -> Self {
        self.session = Some(session);
        self
    }
}

/// The window as it was at the moment of capture.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapturedWindow<'a> {
    pub id: WindowId,
    pub title: Option<&'a str>,
    pub bounds: Bounds,
    pub on_screen: bool,
}

/// The PNG file a capture wrote, in backing pixels. It is `uniform` when every pixel is the same,
/// as in a capture of an invisible or fully transparent window.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapturedImage<'a> {
    pub path: &'a Path,
    pub width: u32,
    pub height: u32,
    pub uniform: bool,
}

/// What `recon ui screenshot` prints.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotResponse<'a> {
    pub application: ObservedApplication<'a>,
    pub window: CapturedWindow<'a>,
    pub image: CapturedImage<'a>,
    /// Every capture result names the mechanism that produced it; there is only one.
    method: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<SessionSummary>,
}

impl<'a> ScreenshotResponse<'a> {
    pub fn new(
        application: ObservedApplication<'a>,
        window: CapturedWindow<'a>,
        image: CapturedImage<'a>,
    ) -> Self {
        Self { application, window, image, method: CAPTURE_METHOD, session: None }
    }

    pub fn with_session(mut self, session: SessionSummary) -> Self {
        self.session = Some(session);
        self
    }
}

// ------------------------------------------------------------- observations

/// One fact and where it came from. A fact Recon could not read is `null` with a reason, which is
/// never the same as a false or empty value.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fact<T> {
    pub value: Option<T>,
    /// What established the fact: the command, interface, or file Recon read.
    pub source: String,
    pub detail: Option<String>,
}

/// The status an observation carries when it has the value it was asked for. It is written out so
/// a reader never has to infer presence from the absence of a status.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ObservedStatus {
    Value,
}

/// Why an observation carries no value. None of these is a false or empty value, and they stay
/// distinct from each other so a reader never collapses "does not exist" into "could not be read".
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UnobservedStatus {
    /// The object has no such relationship or value.
    Absent,
    /// The object's kind does not support the observation.
    Unsupported,
    /// Reading it was refused.
    Denied,
    /// Reading it failed or raised.
    Failed,
    /// The value exists but is not a finite number.
    NonFinite,
    /// AppKit's no-intrinsic-metric sentinel, which is not a measured size.
    NoIntrinsicMetric,
    /// The observation could not be named this time, as when native handle capacity is spent.
    Unavailable,
    /// More than one candidate matched equally well, so no single one is reported.
    Ambiguous,
}

/// Whether a reported value came from a getter that ran application code in the target.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TargetRead {
    TargetExecuted,
}

/// A requested observation that produced no value, and anything the helper added about it.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Unobserved {
    pub status: UnobservedStatus,
    /// What went wrong, when the helper could name it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Present on accessibility reads, which run target code even when they report nothing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read: Option<TargetRead>,
}

/// One requested observation: the value, or the status that explains why there is none. An
/// unrequested observation is absent from the record entirely and carries no status.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum Observed<T> {
    Value(T),
    Unobserved(Unobserved),
}

/// A value the helper wraps with its own status, so a measured zero and an unreadable number are
/// never the same JSON.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Measured<T> {
    pub status: ObservedStatus,
    pub value: T,
}

/// One accessibility property as the object itself reports it, through a getter that runs in the
/// target. It is never `NSView.identifier`, which is a different property.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadText {
    pub status: ObservedStatus,
    pub value: String,
    pub read: TargetRead,
}

/// Which representation produced an observation.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationSource {
    /// In-process AppKit reads through the session's helper.
    NativeAppkit,
}

/// What a result claims about its own consistency. A native read is a best-effort main-thread
/// observation over an interval, not an atomic snapshot of the application or the compositor.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Consistency {
    MainThreadBestEffort,
}

/// Which layer state a result describes. Recon reads model layers; presentation state and
/// animation tracing are not part of this feature.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LayerState {
    Model,
}

/// A kernel-verified process incarnation: the pid together with the generation that distinguishes
/// it from a later process holding the same number.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProcessIncarnation {
    pub pid: Pid,
    pub pidversion: u32,
}

/// The inspected process as it identifies itself from the inside, read in the target rather than
/// inferred from the bundle Recon selected.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetIdentity {
    pub pid: Pid,
    pub pidversion: u32,
    pub uid: u32,
    pub bundle_id: Option<String>,
    /// The bundle the process runs from, which is absent when it runs from a bare executable.
    pub bundle_path: Option<String>,
    pub executable_path: Option<String>,
    pub architecture: String,
    pub os_build: String,
    pub os_version: String,
}

/// What a bounded native walk actually covered. `truncated` says what Recon could not finish and
/// `narrowed` what the caller's own options left out; `complete` is true only when nothing was
/// truncated.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeCoverage {
    /// Objects and relationships visited in the target for this request.
    pub visited: usize,
    /// Records returned to the caller.
    pub returned: usize,
    /// What the caller's own options left out, each named as the helper writes it.
    pub narrowed: Vec<String>,
    /// What stopped the walk, each named with the bound that stopped it.
    pub truncated: Vec<String>,
    pub complete: bool,
}

/// The scope a native read was asked for, echoed so a stored result explains itself.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservedScope {
    pub root: Option<NodeHandle>,
    pub window_id: Option<WindowId>,
    pub depth: usize,
    pub limit: usize,
}

/// The provenance of one native observation: which session read what, through which helper, over
/// which interval, in which process.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeObservation {
    /// A unique id for this observation. It identifies the reading, not any object in it.
    pub observation_id: String,
    pub source: ObservationSource,
    pub session_id: String,
    pub helper_build: String,
    pub protocol: u32,
    pub identity: TargetIdentity,
    pub scope: ObservedScope,
    /// The field groups this read was asked for.
    pub fields: Vec<FieldGroup>,
    /// The kind the read walked or returned, which a traversal selector never relabels.
    pub kind: NodeKind,
    pub consistency: Consistency,
    pub layer_state: LayerState,
    /// Unix epoch seconds bounding the interval the observation covers.
    pub start_time: f64,
    pub end_time: f64,
}

// ----------------------------------------------------------------- sessions

/// The session an observation was read through, and the helper that answered.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSummary {
    pub id: String,
    /// How this session was originally established. Connecting again with another mode does not
    /// rewrite that history.
    pub acquired: AccessMode,
    pub profile: AccessProfile,
    pub protocol_version: u32,
    /// The helper build that is resident in the target.
    pub helper_build: String,
}

/// An effect an open actually performed, as opposed to one the command is allowed to perform.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PerformedEffect {
    /// Helper code is mapped in the target and its bootstrap endpoint is serving.
    HelperBootstrapResident,
}

/// What `recon ui session open` prints: the helper's own account of the open, with the session
/// identity and the reuse decision the host recorded.
///
/// The capability that authenticates later requests is deliberately not here: it stays in the
/// session store and never reaches a printed result.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenResponse {
    pub ok: bool,
    pub session_id: String,
    /// Whether a still-valid session was reused instead of establishing a new one.
    pub reused: bool,
    pub pid: Pid,
    pub pidversion: u32,
    pub uid: u32,
    pub incarnation: ProcessIncarnation,
    pub identity: TargetIdentity,
    pub helper_build: String,
    pub protocol: u32,
    /// The profile the session holds, which a later request cannot change.
    pub profile: AccessProfile,
    pub mode: AccessMode,
    pub requested_mode: AccessMode,
    pub requested_profile: AccessProfile,
    /// How the session was first established, which reconnecting never rewrites.
    pub established_mode: AccessMode,
    /// The operations this session supports, by name.
    pub capabilities: Vec<String>,
    pub effects: Vec<PerformedEffect>,
    /// Whether helper code is mapped in the target now.
    pub helper_resident: bool,
    /// Whether the helper's bootstrap endpoint remains available for later sessions.
    pub bootstrap_active: bool,
}

/// How a confirmed close ended. An acknowledged close and an already-closed session are both
/// successes and are distinguishable.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ConfirmedCloseState {
    /// The helper acknowledged revocation.
    Closed,
    /// The session was already closed, as authenticated state confirms.
    AlreadyClosed,
    /// Authenticated bootstrap state, rather than an acknowledgment, settled the close.
    ClosedAfterReconciliation,
}

/// The state of a close whose acknowledgment never arrived.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UnconfirmedCloseState {
    CloseUnconfirmed,
}

/// A close the helper or authenticated bootstrap state settled. Closing a session never quits an
/// application and never unloads code, so residency is reported rather than claimed to be
/// reversed.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmedClose {
    pub session_id: String,
    pub state: ConfirmedCloseState,
    pub acknowledged: bool,
    /// Whether the target process is gone, which is the only thing that ends residency.
    pub process_exited: bool,
    pub helper_resident: bool,
    pub bootstrap_active: bool,
}

/// A close whose acknowledgment never arrived. Revocation stays uncertain, so residency is
/// reported as unknown rather than guessed.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnconfirmedClose {
    pub session_id: String,
    pub state: UnconfirmedCloseState,
    pub acknowledged: bool,
    pub helper_resident: Option<bool>,
    pub bootstrap_active: Option<bool>,
    /// What stopped the acknowledgment from arriving.
    pub detail: String,
}

/// What `recon ui session close` prints. A missing acknowledgment is neither success nor failure
/// but uncertainty, and says so in its own shape.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum CloseResponse {
    Confirmed(ConfirmedClose),
    Unconfirmed(UnconfirmedClose),
}

// --------------------------------------------------------------- preflight

/// The bundle a report describes, and the instance it was narrowed to. A report about a bundle
/// with no selected instance names no pid rather than inventing one.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectedApplication<'a> {
    pub path: &'a Path,
    pub pid: Option<Pid>,
}

/// Whether one access mode is possible for a target, as static facts can establish.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Eligibility {
    Eligible,
    Blocked,
    /// A fact Recon could not read; not a denial.
    Unknown,
}

/// One mode and profile combination, with the reasons behind its verdict.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModeEligibility {
    pub mode: AccessMode,
    pub profile: AccessProfile,
    pub status: Eligibility,
    pub reasons: Vec<String>,
    pub evidence_sources: Vec<String>,
}

/// The target facts eligibility is read from.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetFacts {
    pub bundle_path: String,
    pub bundle_id: Option<String>,
    pub executable_path: Fact<String>,
    pub pid: Option<Pid>,
    pub running_pids: Vec<Pid>,
    pub process_executable: Fact<String>,
    pub architecture: Fact<String>,
    pub executable_architectures: Fact<Vec<String>>,
    pub signed: Fact<bool>,
    pub apple_signed: Fact<bool>,
    pub get_task_allow: Fact<bool>,
    pub allow_dyld_environment_variables: Fact<bool>,
    pub hardened_runtime: Fact<bool>,
    pub library_validation: Fact<bool>,
    pub platform_binary: Fact<bool>,
}

/// The host facts eligibility is read from. These describe this machine's current posture; Recon
/// changes none of them outside `ui session prepare-host`.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostFacts {
    pub os_version: Fact<String>,
    pub architecture: Fact<String>,
    pub sip_disabled: Fact<bool>,
    pub amfi_disabled: Fact<bool>,
    pub active_boot_args: Fact<Vec<String>>,
    pub configured_boot_args: Fact<Vec<String>>,
}

/// The helper facts eligibility is read from.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HelperFacts {
    pub local_available: Fact<bool>,
    pub local_compatible: Fact<bool>,
    pub resident: Fact<bool>,
    pub resident_compatible: Fact<bool>,
    /// Whether an attach can be attempted without a macOS authorization dialog.
    pub attach_without_dialog: Fact<bool>,
    pub inspector_thread_set_state: Fact<bool>,
}

/// What `recon ui session preflight` prints. A report whose every mode is blocked is a successful
/// result; no system-wide readiness flag substitutes for these target-specific facts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreflightResponse<'a> {
    pub application: SelectedApplication<'a>,
    pub target: TargetFacts,
    pub host: HostFacts,
    pub helper: HelperFacts,
    /// One entry per mode and profile combination, each with its own facts. These are the modes
    /// the facts allow, not a decision about which one to use.
    pub modes: Vec<ModeEligibility>,
}

/// Which host preparation Recon was asked for.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PreparationAction {
    Apply,
    Status,
    Revert,
}

/// What `--status` reports: this host's current posture against the research profile, with
/// nothing changed.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostPreparationStatus {
    /// The preparation this report answers for. A status report names the host's posture rather
    /// than an action, so it carries one only when the request is echoed back.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<PreparationAction>,
    pub host: HostFacts,
    /// The boot arguments the research profile requires, exactly as they are written to nvram.
    pub required_boot_args: String,
    pub readiness: Eligibility,
    /// The exact operator steps Recon cannot perform itself, printed rather than linked.
    pub operator_steps: String,
}

/// What an applied or reverted preparation reports: what changed on this host and what the
/// operator must still do.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostPreparationChange {
    pub action: PreparationAction,
    /// Whether this invocation changed anything on the host.
    pub changed: bool,
    pub reboot_required: bool,
    /// The boot arguments Recon sets or removes, exactly as written.
    pub recon_boot_args: Vec<String>,
    /// The exact operator steps Recon cannot perform itself, printed rather than linked.
    pub operator_steps: Vec<String>,
}

/// What `recon ui session prepare-host` prints. Reporting the host's posture and changing it are
/// different results and keep different shapes.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum PrepareHostResponse {
    /// Boxed because a status report carries every host fact, while a change carries a few lines.
    Status(Box<HostPreparationStatus>),
    Change(HostPreparationChange),
}

// --------------------------------------------------- native visual objects

/// The actual kind of a native visual object. A traversal selector never relabels it.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Window,
    View,
    Layer,
    Cell,
    LayoutGuide,
}

/// Which coordinate system a rectangle's numbers are in.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CoordinateSpace {
    /// The superview's coordinate system, as `NSView.frame` uses.
    Superview,
    /// The object's own coordinate system, as `NSView.bounds` uses.
    #[serde(rename = "self")]
    Own,
    /// The window's coordinate system.
    Window,
    /// AppKit's global screen coordinates.
    GlobalScreen,
    /// The owning view's coordinate system, which a layout guide's frame is in.
    OwningView,
    /// The parent layer's coordinate system.
    ParentModelLayer,
    /// The layer's own coordinate system.
    SelfModelLayer,
}

/// Where a coordinate space's origin sits, recorded separately from the object's own `isFlipped`.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Orientation {
    TopLeft,
    BottomLeft,
    /// The view has no superview, so its frame belongs to no oriented space.
    Unattached,
    /// Core Animation geometry, whose orientation follows the layer's own geometry flipping.
    LayerGeometry,
    Unknown,
}

/// The units a rectangle is measured in. Recon reports logical points and never multiplies them
/// by a backing scale factor.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Units {
    LogicalPoints,
}

/// A rectangle in logical points, with the space it belongs to and that space's orientation.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeRect {
    pub status: ObservedStatus,
    pub space: CoordinateSpace,
    pub units: Units,
    pub orientation: Orientation,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// How a native rectangle was put into the top-left global screen space accessibility reports in.
/// `scale` is metadata about the display; it is reported, never applied to logical points.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenTransform {
    pub status: ObservedStatus,
    pub method: TransformMethod,
    pub orientation: Orientation,
    /// The primary display's height in logical points, which is the whole difference between
    /// AppKit's bottom-left global origin and accessibility's top-left one.
    pub primary_screen_height: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
}

/// The one transformation Recon uses to reach accessibility's origin.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TransformMethod {
    PrimaryScreenHeightFlip,
}

/// The same rectangle in the spaces a caller can compare across sources, reported only when it
/// can actually be computed.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NormalizedFrame {
    /// Window coordinates, which exist only for an object attached to a window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<Observed<NativeRect>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen: Option<Observed<NativeRect>>,
    pub screen_top_left: Observed<NativeRect>,
    pub backing_scale_factor: Observed<Measured<f64>>,
    pub display_id: Observed<u32>,
    pub transform: Observed<ScreenTransform>,
}

/// A recognized SwiftUI hosting boundary and the evidence that recognized it.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostingBoundary {
    pub evidence: String,
    /// What Recon can say about the hosted content: that it recognized a boundary, and no more.
    /// No SwiftUI view values or AttributeGraph relationships are recovered.
    #[serde(rename = "swiftUIContent")]
    pub swift_ui_content: SwiftUiContent,
}

/// How far a SwiftUI boundary was followed.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SwiftUiContent {
    NotInspected,
}

/// The `basic` group: what an object is and where it sits.
///
/// Its members depend on the kind of object observed — a window reports its title and its window
/// number, a layer its layer state — so the named members are the ones Recon knows and the group
/// stays open for members a newer helper reports.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BasicFacet {
    pub status: ObservedStatus,
    /// The object's real runtime class, including private and KVO subclass spellings.
    pub class: String,
    /// The runtime superclass chain, outermost first.
    pub superclasses: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Observed<Measured<String>>>,
    /// `NSView.identifier`, `NSWindow.identifier`, or `NSLayoutGuide.identifier`, which is not the
    /// accessibility identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifier: Option<Observed<Measured<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<Observed<NativeRect>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Observed<NativeRect>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub normalized_frame: Option<Observed<NormalizedFrame>>,
    /// The accessibility interface's own `accessibilityIdentifier`, read through an overridable
    /// getter that runs application code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility_identifier: Option<Observed<ReadText>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility_role: Option<Observed<ReadText>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main: Option<bool>,
    /// The AppKit window-device number, which is source-specific evidence and not a window server
    /// id. A missing number is never replaced by an ordinal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_number: Option<Observed<Measured<i64>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_view: Option<Observed<NodeHandle>>,
    /// The topmost view reachable through the content view's same-window superview chain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topmost_view: Option<Observed<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_windows: Option<Vec<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet: Option<Observed<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superview: Option<Observed<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superlayer: Option<Observed<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owning_view: Option<Observed<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<Observed<NodeHandle>>,
    /// The object's own flipped state, which is not the same as its frame space's orientation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flipped: Option<bool>,
    /// The object's own hidden state. It is observed state, not a claim about visible pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alpha: Option<Observed<Measured<f64>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<Observed<Measured<f64>>>,
    /// How many children the traversed relation has, which can exceed the children returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer_state: Option<LayerState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hosting_boundary: Option<HostingBoundary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub editable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_type: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<i64>,
    /// Members that belong to kinds of object this model does not name yet.
    #[serde(flatten)]
    pub members: Map<String, Value>,
}

/// One color, keeping catalog identity alongside any resolved components.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColorFacts {
    pub status: ObservedStatus,
    pub color_space: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The space the components are in, present only when the conversion succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_space: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<ColorComponents>,
    /// Why a named color kept its identity without components.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conversion: Option<Unobserved>,
}

/// One color's resolved components.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColorComponents {
    pub red: Observed<Measured<f64>>,
    pub green: Observed<Measured<f64>>,
    pub blue: Observed<Measured<f64>>,
    pub alpha: Observed<Measured<f64>>,
}

/// `NSVisualEffectView` facts, which exist only on that kind of view and are reported as the raw
/// AppKit enumerations.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisualEffect {
    pub material: i64,
    pub blending_mode: i64,
    pub state: i64,
    pub emphasized: bool,
}

/// The `appearance` group, whose members depend on the kind of object observed.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceFacet {
    pub status: ObservedStatus,
    /// A view's resolved appearance name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_appearance: Option<Observed<String>>,
    /// A window's own appearance name, which is not a resolved one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Observed<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_color: Option<Observed<ColorFacts>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<Observed<ColorFacts>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<Observed<ColorFacts>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visual_effect: Option<VisualEffect>,
    /// Colors that belong to kinds of view this model does not name yet.
    #[serde(flatten)]
    pub members: Map<String, Value>,
}

/// Which object carried a font. A font inventory inherits the coverage of these carriers.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FontSource {
    View,
    Cell,
    AttributedRun,
}

/// One font as the descriptor reports it. Recon reports what it read; it never converts a trait
/// into a nearest weight name or invents a system font.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontDescriptor {
    pub status: ObservedStatus,
    pub family: String,
    pub post_script_name: String,
    pub point_size: Observed<f64>,
    /// The raw symbolic traits bitmask.
    pub symbolic_traits: u64,
    pub weight: Observed<f64>,
    /// The descriptor's own PostScript name, which can differ from the font's.
    pub descriptor: String,
}

/// The range of one attributed-text run, without its text.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextRange {
    pub location: usize,
    pub length: usize,
}

/// One attributed-text run and the font it carried.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontRun {
    pub range: TextRange,
    pub font: Observed<FontDescriptor>,
    pub source: FontSource,
}

/// The `font` group: the carrier's own font and the bounded attributed-text runs it exposed.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontFacet {
    pub status: ObservedStatus,
    pub carrier: Observed<FontDescriptor>,
    pub source: FontSource,
    pub runs: Vec<FontRun>,
}

/// A reference to another native object: enough to read it with `ui view node`, and never an
/// implicit expansion of it.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObjectReference {
    pub kind: NodeKind,
    pub handle: NodeHandle,
}

/// One layout guide a view owns.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutGuideReference {
    pub kind: NodeKind,
    pub handle: Observed<NodeHandle>,
}

/// One constraint, deduplicated by constraint identity across the ancestor chain. Ownership and
/// reference are different relationships and are reported separately. Attributes and relations
/// keep AppKit's raw enumerations rather than a spelling Recon invents.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Constraint {
    pub owner: Observed<ObjectReference>,
    pub first_item: Observed<ObjectReference>,
    pub second_item: Observed<ObjectReference>,
    pub first_attribute: i64,
    pub second_attribute: i64,
    pub relation: i64,
    pub multiplier: Observed<f64>,
    pub constant: Observed<f64>,
    pub priority: Observed<f64>,
    pub active: bool,
    pub identifier: Observed<String>,
}

/// One priority per layout axis.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AxisPriorities {
    pub horizontal: Observed<f64>,
    pub vertical: Observed<f64>,
}

/// The `layout` group. These are the constraints Recon observed, not a solver explanation.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutFacet {
    pub status: ObservedStatus,
    pub constraints: Vec<Constraint>,
    pub layout_guides: Vec<LayoutGuideReference>,
    /// The intrinsic content size, keeping AppKit's no-intrinsic-metric sentinel distinct from a
    /// measured size.
    pub intrinsic_width: Observed<Measured<f64>>,
    pub intrinsic_height: Observed<Measured<f64>>,
    pub hugging: Observed<AxisPriorities>,
    pub compression_resistance: Observed<AxisPriorities>,
}

/// A model-layer transform as a column-major 4×4 matrix.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayerTransform {
    pub status: ObservedStatus,
    pub matrix: Vec<f64>,
}

/// The `layer` group: the object's existing associated layer and that layer's own facts. It never
/// walks the layer tree; `--kind layer` does that.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayerFacet {
    pub status: ObservedStatus,
    pub layer_state: LayerState,
    pub handle: Observed<NodeHandle>,
    pub frame: Observed<NativeRect>,
    pub bounds: Observed<NativeRect>,
    pub transform: Observed<LayerTransform>,
    pub contents_scale: Observed<f64>,
    pub hidden: bool,
    pub opacity: Observed<f64>,
    pub corner_radius: Observed<f64>,
    /// The raw `CACornerMask` bitmask.
    pub masked_corners: u64,
    pub border_width: Observed<f64>,
    pub shadow_opacity: Observed<f64>,
    pub shadow_radius: Observed<f64>,
    pub masks_to_bounds: bool,
    /// A mask is a distinct relationship, never a sublayer.
    pub mask: Observed<NodeHandle>,
    pub background_color: Observed<ColorFacts>,
    pub border_color: Observed<ColorFacts>,
    pub shadow_color: Observed<ColorFacts>,
}

/// The `cell` group. A cell is a related object of its control, not one of its children.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CellFacet {
    pub status: ObservedStatus,
    pub handle: Observed<NodeHandle>,
    pub class: String,
    /// The raw `NSCellType` and `NSControlStateValue` the cell reports.
    pub cell_type: i64,
    pub state: i64,
    pub enabled: bool,
    pub editable: bool,
    pub font: Observed<FontDescriptor>,
}

/// Relationships a record reports because without them the returned structure cannot be
/// interpreted. Which ones exist depends on the kind of object: a view names its superview and its
/// window, a layer its superlayer. A relationship the object does not have is an explicit absence,
/// not an unread one.
#[derive(Clone, Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Relationships {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superview: Option<Observed<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<Observed<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superlayer: Option<Observed<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_window: Option<Observed<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_view: Option<Observed<NodeHandle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owning_view: Option<Observed<NodeHandle>>,
}

/// How a native window was matched to a window server window. Numeric equality in two identifier
/// domains is not one of these.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
pub enum MappingMethod {
    /// `NSApplication.window(withWindowNumber:)` in the target, checked against the window's owner
    /// pid and `NSApp.windows` membership.
    #[serde(rename = "NSApplication.windowWithWindowNumber")]
    WindowWithWindowNumber,
}

/// A proven relationship between one live native window and one window server window.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowMapping {
    pub method: MappingMethod,
    pub evidence: String,
}

/// One native visual object as a bounded walk observed it.
///
/// Identity and kind are always reported. A record that only refers to an object already returned
/// in this result carries `reference` and nothing else: a repeated object is named, never expanded
/// again. The field groups the request asked for appear by their own names, and a group the
/// object's kind does not support says so rather than disappearing.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeRecord {
    pub handle: NodeHandle,
    pub kind: NodeKind,
    /// Depth below the walk's own root, present on records a traversal returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<usize>,
    /// Set on a record that only refers to an object this result already returned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationships: Option<Observed<Relationships>>,
    /// The window server id this native window was proven to be, present only in a window
    /// inventory. A window with no proven identity keeps its handle instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_id: Option<Observed<WindowId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapping: Option<Observed<WindowMapping>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basic: Option<Observed<BasicFacet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Observed<AppearanceFacet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<Observed<FontFacet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<Observed<LayoutFacet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<Observed<LayerFacet>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell: Option<Observed<CellFacet>>,
}

/// What `recon ui view tree` and `recon ui view find` print. `windowRoots` carries the windows a
/// default forest or a selected window scope walked from; it is a scope record, not a match.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeNodesResponse {
    pub ok: bool,
    pub nodes: Vec<NativeRecord>,
    pub window_roots: Vec<NativeRecord>,
    pub observation: NativeObservation,
    pub coverage: NativeCoverage,
}

/// What `recon ui view node` prints.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeNodeResponse {
    pub ok: bool,
    pub node: NativeRecord,
    pub observation: NativeObservation,
    pub coverage: NativeCoverage,
}

/// One object that carried an observed font.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontCarrier {
    pub handle: NodeHandle,
    pub kind: NodeKind,
    pub source: FontSource,
    /// The attributed-text run the font was observed on, when it was observed on one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<TextRange>,
}

/// One group of identical observed font descriptors and the objects that carried them.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FontInventoryEntry {
    pub descriptor: FontDescriptor,
    pub sources: Vec<FontCarrier>,
}

/// What `recon ui view fonts` prints. A partial walk produces a partial inventory, which the
/// coverage says explicitly.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeFontsResponse {
    pub ok: bool,
    pub fonts: Vec<FontInventoryEntry>,
    pub window_roots: Vec<NativeRecord>,
    pub observation: NativeObservation,
    pub coverage: NativeCoverage,
}

/// One native object as a correlation refers to it. The handle is the helper's spelling; an
/// accessibility path never substitutes for it.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeObjectReference {
    pub handle: NodeHandle,
    pub kind: NodeKind,
    /// The runtime class, which is null when the record carried no `basic` facet.
    pub class: Option<String>,
}

/// One accessibility element, named the way accessibility evidence names things. An accessibility
/// path and a native handle are different identifiers and never substitute for each other.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElementReference {
    pub path: String,
    pub role: Option<String>,
    pub identifier: Option<String>,
}

/// What made two observations candidates for the same interface element.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MatchBasis {
    /// A non-empty exact accessibility identifier observed on both sides.
    AccessibilityIdentifier,
    /// Matching self-reported role and normalized screen bounds within one logical point.
    RoleAndBounds,
}

/// One candidate relationship. Even a unique candidate is evidence, not an assertion that the two
/// observations describe the same underlying object.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorrelationCandidate {
    pub native: NativeObjectReference,
    pub accessibility: ElementReference,
    pub basis: MatchBasis,
    /// Whether either side of this candidate also matched something else on the same basis.
    pub ambiguous: bool,
}

/// What `recon ui correlate` prints. Each side keeps its own full observation, with its own
/// provenance and coverage; unmatched objects are a difference between evidence sources, not an
/// accessibility defect and not an instruction to synthesize anything.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CorrelateResponse<'a> {
    pub native: NativeNodesResponse,
    pub accessibility: TreeResponse<'a>,
    pub candidates: Vec<CorrelationCandidate>,
    pub unmatched_native: Vec<NativeObjectReference>,
    pub unmatched_accessibility: Vec<ElementReference>,
}
