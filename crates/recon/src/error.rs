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
    Internal,
}

impl ErrorCode {
    pub const ALL: [Self; 17] = [
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
        Self::Internal,
    ];

    pub fn exit_code(self) -> i32 {
        if self == Self::InvalidArgument { 2 } else { 1 }
    }

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
