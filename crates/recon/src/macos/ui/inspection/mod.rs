//! Execution for native inspection sessions and visual reads.

mod artifact;
mod native;
mod session;

use std::io::{self, Write};

use serde_json::{Value, json};

pub(super) use native::{native_windows, validate_session, view};
pub(super) use session::{close_session, open_session};

use crate::application::{Application, Pid};
use crate::error::{ErrorCode, Failure, InspectionFailure, InspectionStage};
use crate::inspection::{InspectionError, InspectionErrorCode, SessionId, Stage};
use crate::macos::inspection_access::{self, SystemRunner};
use crate::ui::request::HostPreparation;

pub(super) fn preflight(
    application: &Application,
    pid: Option<Pid>,
) -> Result<Value, InspectionError> {
    let mut report = session::preflight(application, pid)?;
    let Some(object) = report.as_object_mut() else {
        return Err(InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Preflight,
            "preflight did not produce an object",
        ));
    };
    object.insert(
        "application".to_owned(),
        json!({ "path": application.path, "pid": pid.map(|pid| pid.0) }),
    );
    if let Some(decisions) = object.remove("decisions") {
        object.insert("modes".to_owned(), decisions);
    }
    Ok(report)
}

pub(super) fn prepare_host(action: HostPreparation) -> Result<Value, InspectionError> {
    let mut runner = SystemRunner;
    match action {
        HostPreparation::Status => {
            let mut value = serialize(inspection_access::prepare_host_status(&mut runner))?;
            value["action"] = Value::from("status");
            Ok(value)
        }
        HostPreparation::Apply => {
            let status = inspection_access::prepare_host_status(&mut runner);
            if status.host.configured_boot_args.value.is_none() {
                return Err(InspectionError::new(
                    InspectionErrorCode::Unavailable,
                    Stage::Preflight,
                    "current boot-args are unreadable, so Recon cannot plan a safe host change",
                ));
            }
            eprintln!(
                "Recon will set these research boot-args through sudo nvram: {}\n{}",
                status.required_boot_args, status.operator_steps
            );
            eprint!("Type `prepare-host` to apply these changes: ");
            io::stderr().flush().map_err(|error| {
                InspectionError::from_io(
                    InspectionErrorCode::Failed,
                    Stage::Preflight,
                    "the confirmation prompt could not be written",
                    &error,
                )
            })?;
            let mut confirmation = String::new();
            io::stdin().read_line(&mut confirmation).map_err(|error| {
                InspectionError::from_io(
                    InspectionErrorCode::Failed,
                    Stage::Preflight,
                    "the confirmation could not be read",
                    &error,
                )
            })?;
            if confirmation.trim() != "prepare-host" {
                return Err(InspectionError::new(
                    InspectionErrorCode::InvalidArgument,
                    Stage::Preflight,
                    "host preparation was not confirmed; no changes were made",
                ));
            }
            let changed = inspection_access::prepare_host(&mut runner).map_err(|error| {
                InspectionError::new(InspectionErrorCode::Unavailable, Stage::Attach, error)
            })?;
            preparation_value("apply", changed)
        }
        HostPreparation::Revert => {
            let changed = inspection_access::revert_host(&mut runner).map_err(|error| {
                InspectionError::new(InspectionErrorCode::Unavailable, Stage::Attach, error)
            })?;
            preparation_value("revert", changed)
        }
    }
}

fn preparation_value(
    action: &str,
    preparation: inspection_access::HostPreparation,
) -> Result<Value, InspectionError> {
    Ok(json!({
        "action": action,
        "changed": true,
        "rebootRequired": true,
        "reconBootArgs": preparation.boot_args.split_whitespace().collect::<Vec<_>>(),
        "operatorSteps": [preparation.operator_steps],
    }))
}

fn serialize(value: impl serde::Serialize) -> Result<Value, InspectionError> {
    serde_json::to_value(value).map_err(|error| {
        InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Preflight,
            format!("the host-preparation result could not be encoded: {error}"),
        )
    })
}

pub(super) fn failure(
    error: &InspectionError,
    session: Option<&SessionId>,
    pid: Option<Pid>,
) -> Failure {
    let code = match error.code() {
        InspectionErrorCode::Unavailable => ErrorCode::InspectionUnavailable,
        InspectionErrorCode::ProtocolMismatch => ErrorCode::InspectionProtocolMismatch,
        InspectionErrorCode::Failed => ErrorCode::InspectionFailed,
        InspectionErrorCode::Timeout => ErrorCode::InspectionTimeout,
        InspectionErrorCode::Busy => ErrorCode::InspectionBusy,
        InspectionErrorCode::StaleSession => ErrorCode::StaleSession,
        InspectionErrorCode::StaleNode => ErrorCode::StaleNode,
        InspectionErrorCode::InvalidArgument => ErrorCode::InvalidArgument,
    };
    let stage = match error.stage() {
        Stage::Preflight => InspectionStage::Preflight,
        Stage::Attach => InspectionStage::Attach,
        Stage::Store => InspectionStage::Store,
        Stage::Connect => InspectionStage::Connect,
        Stage::Peer => InspectionStage::Peer,
        Stage::Handshake => InspectionStage::Handshake,
        Stage::Send => InspectionStage::Send,
        Stage::Receive => InspectionStage::Receive,
        Stage::Read => InspectionStage::Read,
        Stage::Close => InspectionStage::Close,
        Stage::Reconcile => InspectionStage::Reconcile,
    };
    let mut details = InspectionFailure::new(stage).detail(error.message().to_owned());
    if let Some(session) = session {
        details = details.session(session.as_str());
    }
    if let Some(pid) = pid {
        details = details.pid(pid.0);
    }
    if error.may_continue_in_target() {
        details = details.target_execution_may_continue();
    }
    if matches!(error.stage(), Stage::Attach | Stage::Read | Stage::Close | Stage::Reconcile) {
        details = details.helper_may_remain_resident();
    }
    Failure::new(code, error.message().to_owned(), details.details())
}
