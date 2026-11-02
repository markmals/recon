//! Execution for native inspection sessions and visual reads.

use std::io::{self, Write};
use std::path::Path;

use serde_json::{Value, json};

use crate::access::{self, SystemRunner};
use crate::artifact;
use crate::identity::{AccessProfile, SessionId};
use crate::lifecycle;
use crate::request::HostPreparation;
use crate::staging::{self, STAGED_DIRECTORY};
use crate::{InspectionError, InspectionErrorCode, Stage};
use recon_applications_macos::application::Application;
use recon_core::error::{ErrorCode, Failure, InspectionFailure, InspectionStage};
use recon_core::process::Pid;

pub fn preflight(application: &Application, pid: Option<Pid>) -> Result<Value, InspectionError> {
    let mut report = lifecycle::preflight(application, pid)?;
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

pub fn prepare_host(action: HostPreparation) -> Result<Value, InspectionError> {
    let mut runner = SystemRunner;
    let staged_directory = Path::new(STAGED_DIRECTORY);
    match action {
        HostPreparation::Status => {
            let packaged = artifact::locate(AccessProfile::Standard).ok();
            let staged = staging::staged_fact(packaged.as_ref(), staged_directory);
            let mut value = serialize(access::prepare_host_status(&mut runner, staged))?;
            value["action"] = Value::from("status");
            Ok(value)
        }
        HostPreparation::Apply => {
            let packaged = artifact::locate(AccessProfile::Standard)?;
            let staged = staging::staged_fact(Some(&packaged), staged_directory);
            let status = access::prepare_host_status(&mut runner, staged);
            if status.host.configured_boot_args.value.is_none() {
                return Err(InspectionError::new(
                    InspectionErrorCode::Unavailable,
                    Stage::Preflight,
                    "current boot-args are unreadable, so Recon cannot plan a safe host change",
                ));
            }
            eprintln!(
                "Recon will set these research boot-args through sudo nvram: {}\nIt will also \
                 install its research helper (build {}) in {} through sudo install, because App \
                 Sandboxed applications can load a helper only from a location like it.\n{}",
                status.required_boot_args,
                packaged.helper_build,
                STAGED_DIRECTORY,
                status.operator_steps
            );
            confirm()?;
            let store = lifecycle::default_store()?;
            let mut changed =
                access::prepare_host(&mut runner, store.root(), &status.required_boot_args)
                    .map_err(|error| {
                        InspectionError::new(InspectionErrorCode::Unavailable, Stage::Attach, error)
                    })?;
            let helper =
                staging::install(&mut runner, &packaged, staged_directory).map_err(|error| {
                    InspectionError::new(
                        InspectionErrorCode::Unavailable,
                        Stage::Attach,
                        format!(
                            "the research boot-args were set, but the research helper could not \
                             be installed in {STAGED_DIRECTORY} ({error}); run prepare-host again \
                             to install it"
                        ),
                    )
                })?;
            changed.staged_helper = Some(helper.display().to_string());
            preparation_value("apply", changed)
        }
        HostPreparation::Revert => {
            let store = lifecycle::default_store()?;
            let mut changed = access::revert_host(&mut runner, store.root()).map_err(|error| {
                InspectionError::new(InspectionErrorCode::Unavailable, Stage::Attach, error)
            })?;
            let removed = staging::remove(&mut runner, staged_directory).map_err(|error| {
                InspectionError::new(
                    InspectionErrorCode::Unavailable,
                    Stage::Attach,
                    format!(
                        "the boot-args were reverted, but the research helper in \
                         {STAGED_DIRECTORY} could not be removed ({error})"
                    ),
                )
            })?;
            if removed {
                changed.changed = true;
                changed.staged_helper =
                    Some(staged_directory.join(artifact::HELPER_FILE).display().to_string());
            }
            preparation_value("revert", changed)
        }
    }
}

fn confirm() -> Result<(), InspectionError> {
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
    if confirmation.trim() == "prepare-host" {
        Ok(())
    } else {
        Err(InspectionError::new(
            InspectionErrorCode::InvalidArgument,
            Stage::Preflight,
            "host preparation was not confirmed; no changes were made",
        ))
    }
}

fn preparation_value(
    action: &str,
    preparation: access::HostPreparation,
) -> Result<Value, InspectionError> {
    Ok(json!({
        "action": action,
        "changed": preparation.changed,
        "rebootRequired": preparation.reboot_required,
        "reconBootArgs": preparation.boot_args.split_whitespace().collect::<Vec<_>>(),
        "stagedHelper": preparation.staged_helper,
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

pub fn failure(error: &InspectionError, session: Option<&SessionId>, pid: Option<Pid>) -> Failure {
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

#[cfg(test)]
mod preparation_tests {
    use super::*;

    #[test]
    fn revert_with_no_record_reports_no_change_or_reboot() {
        let preparation = access::HostPreparation {
            changed: false,
            reboot_required: false,
            boot_args: "keepsyms=1".into(),
            staged_helper: None,
            operator_steps: "No Recon record was found; nothing was reverted.".into(),
        };
        let output = preparation_value("revert", preparation).expect("one response");
        assert_eq!(output["changed"], false);
        assert_eq!(output["rebootRequired"], false);
        assert_eq!(output["reconBootArgs"], json!(["keepsyms=1"]));
    }
}
