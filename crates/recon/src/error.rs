use std::fmt;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::{Value, json};

mod schema;
#[cfg(test)]
mod test_contract;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidArgument,
    UnsupportedHost,
    DiscoveryUnavailable,
    NotFound,
    NotAnApplication,
    AmbiguousTarget,
    AmbiguousInstance,
    NotRunning,
    PermissionDenied,
    IdentityUnavailable,
    InterfaceUnavailable,
    Timeout,
    CaptureFailed,
    LaunchFailed,
    AlreadyExists,
    IoFailed,
    /// Inspection access is not possible for this target, host, or mode: an unsupported target
    /// ABI or build, or a missing access prerequisite the message names.
    InspectionUnavailable,
    /// The resident helper or its wire protocol does not match this Recon build.
    InspectionProtocolMismatch,
    /// An inspection attempt failed at a named stage.
    InspectionFailed,
    /// A native read or a session lifecycle step exceeded its deadline. Target execution may
    /// continue; nothing was rolled back.
    InspectionTimeout,
    /// The process already has outstanding inspection work, or an uncertain close to reconcile.
    InspectionBusy,
    /// A well-formed session identity no longer names a live session.
    StaleSession,
    /// A well-formed handle no longer names a live native object.
    StaleNode,
    /// Recon observed a violation of the no-focus contract while running the operation.
    UiInterference,
    Internal,
}

impl ErrorCode {
    pub const ALL: [Self; 25] = [
        Self::InvalidArgument,
        Self::UnsupportedHost,
        Self::DiscoveryUnavailable,
        Self::NotFound,
        Self::NotAnApplication,
        Self::AmbiguousTarget,
        Self::AmbiguousInstance,
        Self::NotRunning,
        Self::PermissionDenied,
        Self::IdentityUnavailable,
        Self::InterfaceUnavailable,
        Self::Timeout,
        Self::CaptureFailed,
        Self::LaunchFailed,
        Self::AlreadyExists,
        Self::IoFailed,
        Self::InspectionUnavailable,
        Self::InspectionProtocolMismatch,
        Self::InspectionFailed,
        Self::InspectionTimeout,
        Self::InspectionBusy,
        Self::StaleSession,
        Self::StaleNode,
        Self::UiInterference,
        Self::Internal,
    ];

    pub fn exit_code(self) -> i32 {
        if self == Self::InvalidArgument { 2 } else { 1 }
    }

    /// Whether running the same command again can succeed without the caller changing anything.
    ///
    /// No native classification is retryable. An inspection timeout leaves target execution that
    /// Recon cannot cancel, a failed attach may have left effects, and a busy process needs the
    /// named recovery action rather than another attempt.
    pub fn retryable(self) -> bool {
        matches!(self, Self::DiscoveryUnavailable | Self::Timeout | Self::CaptureFailed)
    }

    pub fn summary(self) -> &'static str {
        match self {
            Self::InvalidArgument => "The command line is invalid",
            Self::UnsupportedHost => "The host operating system is unsupported",
            Self::DiscoveryUnavailable => "Spotlight application discovery is unavailable",
            Self::NotFound => "The requested application, path, instance, or window was not found",
            Self::NotAnApplication => "The path is not a readable application bundle",
            Self::AmbiguousTarget => "The selector matches multiple application bundles",
            Self::AmbiguousInstance => "The bundle has multiple running instances",
            Self::NotRunning => "The selected application is not running",
            Self::PermissionDenied => "Recon lacks a required privacy permission",
            Self::IdentityUnavailable => "Recon cannot establish its own privacy identity",
            Self::InterfaceUnavailable => "A required private interface is unavailable",
            Self::Timeout => "An accessibility request timed out",
            Self::CaptureFailed => "Window capture or PNG encoding failed",
            Self::LaunchFailed => "macOS could not launch the selected application",
            Self::AlreadyExists => "The output file already exists",
            Self::IoFailed => "A filesystem operation failed",
            Self::InspectionUnavailable => {
                "Native inspection access is unavailable for this target, host, or mode"
            }
            Self::InspectionProtocolMismatch => {
                "The resident helper or wire protocol does not match this Recon build"
            }
            Self::InspectionFailed => "A native inspection attempt failed",
            Self::InspectionTimeout => "A native inspection request exceeded its deadline",
            Self::InspectionBusy => {
                "The process has outstanding inspection work or an unresolved close"
            }
            Self::StaleSession => "The session identifier no longer names a live session",
            Self::StaleNode => "The handle no longer names a live native object",
            Self::UiInterference => "The operation disturbed the user's foreground work",
            Self::Internal => "An unexpected internal failure occurred",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    pub code: ErrorCode,
    pub message: String,
    pub retryable: bool,
    pub command: Option<Vec<String>>,
    #[schemars(schema_with = "schema::details")]
    pub details: Option<Value>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ErrorReport<'a> {
    pub error: &'a Failure,
}

impl Failure {
    pub fn new(code: ErrorCode, message: impl Into<String>, details: Value) -> Self {
        let failure = Self {
            code,
            message: message.into(),
            retryable: code.retryable(),
            command: None,
            details: (!details.is_null()).then_some(details),
        };
        #[cfg(test)]
        test_contract::validate(&failure);
        failure
    }

    pub fn internal(message: impl Into<String>) -> Self {
        let message = message.into();
        let details = json!({"detail":message});
        Self::new(ErrorCode::Internal, message, details)
    }

    pub fn with_command(mut self, command: &[&str]) -> Self {
        self.command = Some(command.iter().map(|word| (*word).to_owned()).collect());
        #[cfg(test)]
        test_contract::validate(&self);
        self
    }

    pub fn exit_code(&self) -> i32 {
        self.code.exit_code()
    }

    pub fn report(&self) -> ErrorReport<'_> {
        ErrorReport { error: self }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for Failure {}

#[derive(Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotFound<'a> {
    #[schemars(schema_with = "schema::not_found_kind")]
    pub kind: &'a str,
    pub selector: Option<&'a str>,
    pub path: Option<String>,
    pub pid: Option<u32>,
    pub window_id: Option<u32>,
    pub running_pids: Option<&'a [crate::application::Pid]>,
    pub detail: Option<String>,
}

impl NotFound<'_> {
    pub fn details(self) -> Value {
        serde_json::to_value(self).expect("not-found details serialize")
    }
}

/// Which part of an inspection attempt a failure happened in.
///
/// The stage is what tells a caller whether anything reached the target: a refused descriptor and
/// a half-finished main-thread read are both failures, but only one can leave work running.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum InspectionStage {
    /// Reading target, host, and helper facts before any access is attempted.
    Preflight,
    /// Binding the loader to the verified incarnation and loading the helper.
    Attach,
    /// Establishing or reusing a session.
    Open,
    /// Reading, creating, or validating the owner-private descriptor store.
    Store,
    /// Opening the local endpoint.
    Connect,
    /// Verifying the peer's kernel-reported identity.
    Peer,
    /// The handshake and its compatibility checks.
    Handshake,
    /// Writing a request frame.
    Send,
    /// Reading and decoding a response frame.
    Receive,
    /// Target-side evaluation of a native read.
    Read,
    /// Revoking a session.
    Close,
    /// Resolving an uncertain close against authenticated bootstrap state.
    Reconcile,
}

/// The details of a native inspection failure: where it happened, which target and session it was
/// about, and what may still be true in the target afterwards.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InspectionFailure<'a> {
    pub stage: InspectionStage,
    /// The session the request named, when the failure is about one.
    pub session: Option<&'a str>,
    pub pid: Option<u32>,
    pub window_id: Option<u32>,
    /// The native handle the request named, when the failure is about one.
    pub node: Option<&'a str>,
    /// The observed cause, written for an operator and never carrying a session capability.
    pub detail: Option<String>,
    /// Whether work Recon dispatched into the target may still be running. Recon never kills the
    /// target or unwinds its main thread, so this is not a rollback claim.
    pub target_execution_may_continue: bool,
    /// Whether helper code may remain mapped in the target. Closing a session is not unloading.
    pub helper_may_remain_resident: bool,
    /// An exact action that resolves the failure, such as the close that reconciles an uncertain
    /// revocation. It is advice, not an automatic retry.
    pub recovery: Option<&'a str>,
}

impl<'a> InspectionFailure<'a> {
    pub fn new(stage: InspectionStage) -> Self {
        Self {
            stage,
            session: None,
            pid: None,
            window_id: None,
            node: None,
            detail: None,
            target_execution_may_continue: false,
            helper_may_remain_resident: false,
            recovery: None,
        }
    }

    pub fn session(mut self, session: &'a str) -> Self {
        self.session = Some(session);
        self
    }

    pub fn pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }

    pub fn window(mut self, window_id: u32) -> Self {
        self.window_id = Some(window_id);
        self
    }

    pub fn node(mut self, handle: &'a str) -> Self {
        self.node = Some(handle);
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn target_execution_may_continue(mut self) -> Self {
        self.target_execution_may_continue = true;
        self
    }

    pub fn helper_may_remain_resident(mut self) -> Self {
        self.helper_may_remain_resident = true;
        self
    }

    pub fn recovery(mut self, recovery: &'a str) -> Self {
        self.recovery = Some(recovery);
        self
    }

    pub fn details(self) -> Value {
        serde_json::to_value(self).expect("inspection failure details serialize")
    }
}

/// What Recon observed when it detected a violation of the no-focus contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum InterferenceEvidence {
    /// The frontmost application changed.
    ForegroundApplication,
    /// The key window or the window order changed.
    KeyWindow,
    /// The real pointer moved.
    PointerLocation,
    /// The active Space changed.
    SpaceChange,
    /// Input was routed somewhere other than the caller intended.
    InputRouting,
    /// The target itself reported an activation or presentation.
    TargetReport,
}

/// The details of a detected no-focus violation. A disturbance is a failed operation with observed
/// effects, not a successful result with a warning.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Interference<'a> {
    pub stage: InspectionStage,
    pub evidence_source: InterferenceEvidence,
    pub pid: Option<u32>,
    pub window_id: Option<u32>,
    /// The observed disturbance, as Recon saw it. An unexplained concurrent user action is not
    /// attributed to the target.
    pub disturbance: Option<String>,
    pub session: Option<&'a str>,
    pub target_execution_may_continue: bool,
}

impl<'a> Interference<'a> {
    pub fn new(stage: InspectionStage, evidence_source: InterferenceEvidence) -> Self {
        Self {
            stage,
            evidence_source,
            pid: None,
            window_id: None,
            disturbance: None,
            session: None,
            target_execution_may_continue: false,
        }
    }

    pub fn pid(mut self, pid: u32) -> Self {
        self.pid = Some(pid);
        self
    }

    pub fn window(mut self, window_id: u32) -> Self {
        self.window_id = Some(window_id);
        self
    }

    pub fn disturbance(mut self, disturbance: impl Into<String>) -> Self {
        self.disturbance = Some(disturbance.into());
        self
    }

    pub fn session(mut self, session: &'a str) -> Self {
        self.session = Some(session);
        self
    }

    pub fn target_execution_may_continue(mut self) -> Self {
        self.target_execution_may_continue = true;
        self
    }

    pub fn details(self) -> Value {
        serde_json::to_value(self).expect("interference details serialize")
    }
}
