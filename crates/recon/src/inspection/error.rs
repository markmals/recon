//! The closed native-inspection error vocabulary shared by the store, the transport, and the
//! session lifecycle.

use std::fmt;
use std::io;

use serde_json::Value;

/// Proposal 0005's closed native classifications. A helper code outside this set is reported as
/// [`InspectionErrorCode::Failed`] rather than passed through, so the published contract cannot be
/// widened by a target-resident helper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionErrorCode {
    Unavailable,
    ProtocolMismatch,
    Failed,
    Timeout,
    Busy,
    StaleSession,
    StaleNode,
    InvalidArgument,
}

impl InspectionErrorCode {
    /// The published spelling, which is also the wire spelling.
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Unavailable => "INSPECTION_UNAVAILABLE",
            Self::ProtocolMismatch => "INSPECTION_PROTOCOL_MISMATCH",
            Self::Failed => "INSPECTION_FAILED",
            Self::Timeout => "INSPECTION_TIMEOUT",
            Self::Busy => "INSPECTION_BUSY",
            Self::StaleSession => "STALE_SESSION",
            Self::StaleNode => "STALE_NODE",
            Self::InvalidArgument => "INVALID_ARGUMENT",
        }
    }

    pub fn from_wire(code: &str) -> Option<Self> {
        match code {
            "INSPECTION_UNAVAILABLE" => Some(Self::Unavailable),
            "INSPECTION_PROTOCOL_MISMATCH" => Some(Self::ProtocolMismatch),
            "INSPECTION_FAILED" => Some(Self::Failed),
            "INSPECTION_TIMEOUT" => Some(Self::Timeout),
            "INSPECTION_BUSY" => Some(Self::Busy),
            "STALE_SESSION" => Some(Self::StaleSession),
            "STALE_NODE" => Some(Self::StaleNode),
            "INVALID_ARGUMENT" => Some(Self::InvalidArgument),
            _ => None,
        }
    }
}

impl fmt::Display for InspectionErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_wire())
    }
}

/// Which part of an inspection attempt failed. A caller needs this to tell a refused local
/// descriptor from a target-side read, because only the latter can leave work running in the
/// target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Stage {
    /// Reading target, host, and helper facts before any access is attempted.
    Preflight,
    /// Binding the loader and starting the helper in the selected incarnation.
    Attach,
    /// Reading, creating, or validating the owner-private descriptor root and its records.
    Store,
    /// Opening the local endpoint.
    Connect,
    /// Verifying the peer's kernel-reported identity.
    Peer,
    /// The open handshake and its compatibility checks.
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

impl Stage {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Preflight => "preflight",
            Self::Attach => "attach",
            Self::Store => "store",
            Self::Connect => "connect",
            Self::Peer => "peer",
            Self::Handshake => "handshake",
            Self::Send => "send",
            Self::Receive => "receive",
            Self::Read => "read",
            Self::Close => "close",
            Self::Reconcile => "reconcile",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_wire())
    }
}

/// One native-inspection failure: its classification, the stage that produced it, and whether
/// target-side execution may still be running.
///
/// `message` is written for an operator and must never carry a session capability; the only way to
/// read a capability is [`crate::inspection::Capability::expose`], which nothing here calls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionError {
    code: InspectionErrorCode,
    stage: Stage,
    message: String,
    target_execution_may_continue: bool,
}

impl InspectionError {
    pub fn new(code: InspectionErrorCode, stage: Stage, message: impl Into<String>) -> Self {
        Self { code, stage, message: message.into(), target_execution_may_continue: false }
    }

    /// Mark a failure that left work dispatched into the target. Recon never kills the target or
    /// unwinds its main thread, so a timed-out read is not a rollback.
    pub fn target_execution_may_continue(mut self) -> Self {
        self.target_execution_may_continue = true;
        self
    }

    pub fn from_io(
        code: InspectionErrorCode,
        stage: Stage,
        context: &str,
        error: &io::Error,
    ) -> Self {
        Self::new(code, stage, format!("{context}: {error}"))
    }

    /// Decode a helper's `{ "ok": false, "error": { ... } }` payload. An unknown or missing code is
    /// an `INSPECTION_FAILED` protocol violation, not a new classification.
    pub fn from_wire(stage: Stage, error: &Value) -> Self {
        let code = error
            .get("code")
            .and_then(Value::as_str)
            .and_then(InspectionErrorCode::from_wire)
            .unwrap_or(InspectionErrorCode::Failed);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("the helper reported a failure without a message");
        let stage = error
            .get("stage")
            .and_then(Value::as_str)
            .and_then(|reported| match reported {
                "preflight" => Some(Stage::Preflight),
                "attach" => Some(Stage::Attach),
                "store" => Some(Stage::Store),
                "connect" => Some(Stage::Connect),
                "peer" => Some(Stage::Peer),
                "handshake" => Some(Stage::Handshake),
                "send" => Some(Stage::Send),
                "receive" => Some(Stage::Receive),
                "read" | "target-execution" | "main-thread-queue" => Some(Stage::Read),
                "close" => Some(Stage::Close),
                "reconcile" => Some(Stage::Reconcile),
                _ => None,
            })
            .unwrap_or(stage);
        let continues =
            error.get("targetExecutionMayContinue").and_then(Value::as_bool).unwrap_or(false);
        Self { code, stage, message: message.to_owned(), target_execution_may_continue: continues }
    }

    pub fn code(&self) -> InspectionErrorCode {
        self.code
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn may_continue_in_target(&self) -> bool {
        self.target_execution_may_continue
    }

    /// Whether the failure leaves the helper's view of a session unknown. Such a failure turns a
    /// close into `close-unconfirmed` instead of an invented success.
    pub fn leaves_outcome_unknown(&self) -> bool {
        matches!(
            self.code,
            InspectionErrorCode::Timeout
                | InspectionErrorCode::Failed
                | InspectionErrorCode::Unavailable
                | InspectionErrorCode::ProtocolMismatch
        )
    }
}

impl fmt::Display for InspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at {}: {}", self.code, self.stage, self.message)
    }
}

impl std::error::Error for InspectionError {}
