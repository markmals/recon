//! Session establishment against an exact application and process incarnation.

use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::artifact::{self, Artifacts};
use crate::application::{Application, Pid};
use crate::inspection::{
    AccessProfile, AcquisitionMode, CloseOutcome, Incarnation, InspectionError,
    InspectionErrorCode, OpenRequest, SessionId, SessionState, SessionStore, Stage, close, open,
};
use crate::macos::inject;
use crate::macos::inspection_access::{
    self, AccessMode, Eligibility, Fact, HelperFacts, SystemRunner, TargetSelection,
};
use crate::macos::workspace;
use crate::ui::request::Selection;

pub(crate) fn preflight(
    application: &Application,
    pid: Option<Pid>,
) -> Result<Value, InspectionError> {
    let pid =
        pid.or_else(|| (application.running_pids.len() == 1).then(|| application.running_pids[0]));
    let artifacts = artifact::locate(AccessProfile::Standard).ok();
    let helper = helper_facts(pid, artifacts.as_ref());
    let mut runner = SystemRunner;
    let report = inspection_access::collect_preflight(
        &mut runner,
        target_selection(application, pid),
        helper,
    );
    serde_json::to_value(report).map_err(|error| {
        InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Preflight,
            format!("the preflight report could not be encoded: {error}"),
        )
    })
}

pub(crate) fn open_session(
    application: &Application,
    selection: &Selection,
    mode: AcquisitionMode,
    profile: AccessProfile,
    allow_authorization: bool,
) -> Result<Value, InspectionError> {
    let artifacts = artifact::locate(profile)?;
    if mode == AcquisitionMode::Launch && !application.running_pids.is_empty() {
        return Err(InspectionError::new(
            InspectionErrorCode::Busy,
            Stage::Attach,
            "launch mode refuses an already-running application; use existing or attach",
        ));
    }
    let pid = match mode {
        AcquisitionMode::Launch => {
            check_access(application, None, mode, profile, allow_authorization, &artifacts)?;
            workspace::launch_background(&application.path).map_err(|error| {
                InspectionError::new(
                    InspectionErrorCode::Unavailable,
                    Stage::Attach,
                    format!("background application launch failed: {error}"),
                )
            })?
        }
        AcquisitionMode::Existing | AcquisitionMode::Attach => {
            let pid = crate::ui::instance::select_instance(
                &application.path,
                &application.running_pids,
                selection.pid,
            )
            .map_err(|error| {
                InspectionError::new(
                    InspectionErrorCode::Unavailable,
                    Stage::Attach,
                    format!("the running instance could not be selected: {error}"),
                )
            })?;
            check_access(application, Some(pid), mode, profile, allow_authorization, &artifacts)?;
            pid
        }
    };

    let target = inject::bind_target(pid.0 as i32).map_err(|error| {
        InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Attach,
            format!("the selected process incarnation could not be bound: {error}"),
        )
    })?;
    let incarnation = Incarnation::from(target.handle.incarnation());
    let store = default_store()?;
    if let Some(record) = store.live_record_for(incarnation)?
        && !same_bundle(&record, application)
    {
        return Err(InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Handshake,
            "the selected session belongs to a different bundle or bundle location",
        ));
    }
    let socket = store.endpoint_path(incarnation)?;
    let bundle_id = application.bundle_id.as_deref().ok_or_else(|| {
        InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Attach,
            "the selected application has no bundle identifier",
        )
    })?;
    let request = OpenRequest {
        socket_path: &socket,
        bundle_path: &application.path,
        bundle_id,
        incarnation,
        mode,
        profile,
        helper_build: &artifacts.helper_build,
        protocol: 1,
    };

    if mode != AcquisitionMode::Launch {
        match open(&store, &request) {
            Ok(outcome) => {
                return session_value(outcome.record.session_id, outcome.response, outcome.reused);
            }
            Err(error) if mode == AcquisitionMode::Attach && error.stage() == Stage::Connect => {}
            Err(error) => return Err(error),
        }
    }

    remove_refused_endpoint(&socket)?;
    artifact::load(&artifacts, incarnation, &socket)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match open(&store, &request) {
            Ok(outcome) => {
                return session_value(outcome.record.session_id, outcome.response, outcome.reused);
            }
            Err(error) if error.stage() == Stage::Connect && Instant::now() < deadline => {
                sleep(Duration::from_millis(20));
            }
            Err(error) => return Err(error),
        }
    }
}

pub(crate) fn close_session(session: &SessionId) -> Result<Value, InspectionError> {
    let store = default_store()?;
    let outcome = close(&store, session)?;
    let (state, acknowledged, process_exited) = match outcome {
        CloseOutcome::Acknowledged { was_active } => {
            (if was_active { "closed" } else { "already-closed" }, true, false)
        }
        CloseOutcome::AlreadyClosed => ("already-closed", true, false),
        CloseOutcome::ReconciledClosed { process_exited } => {
            ("closed-after-reconciliation", true, process_exited)
        }
        CloseOutcome::Unconfirmed(error) => {
            return Ok(json!({
                "sessionId": session.as_str(),
                "state": "close-unconfirmed",
                "acknowledged": false,
                "helperResident": Value::Null,
                "bootstrapActive": Value::Null,
                "detail": error.to_string(),
            }));
        }
    };
    Ok(json!({
        "sessionId": session.as_str(),
        "state": state,
        "acknowledged": acknowledged,
        "processExited": process_exited,
        "helperResident": !process_exited,
        "bootstrapActive": !process_exited,
    }))
}

fn check_access(
    application: &Application,
    pid: Option<Pid>,
    mode: AcquisitionMode,
    profile: AccessProfile,
    allow_authorization: bool,
    artifacts: &Artifacts,
) -> Result<(), InspectionError> {
    let helper = helper_facts(pid, Some(artifacts));
    let target = target_selection(application, pid);
    let facts = inspection_access::collect_preflight(&mut SystemRunner, target, helper);
    let expected_mode = match mode {
        AcquisitionMode::Existing => AccessMode::Existing,
        AcquisitionMode::Launch => AccessMode::Launch,
        AcquisitionMode::Attach => AccessMode::Attach,
    };
    let expected_profile = match profile {
        AccessProfile::Standard => inspection_access::AccessProfile::Standard,
        AccessProfile::Research => inspection_access::AccessProfile::Research,
    };
    let mut decisions = inspection_access::evaluate_with_authorization(
        &facts.target,
        &facts.host,
        &facts.helper,
        allow_authorization,
    );
    let decision = decisions
        .drain(..)
        .find(|decision| decision.mode == expected_mode && decision.profile == expected_profile)
        .ok_or_else(|| {
            InspectionError::new(
                InspectionErrorCode::Failed,
                Stage::Preflight,
                "preflight omitted the requested mode and profile",
            )
        })?;
    if decision.status != Eligibility::Blocked {
        Ok(())
    } else {
        Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Preflight,
            format!("inspection access is {:?}: {}", decision.status, decision.reasons.join("; ")),
        ))
    }
}

fn helper_facts(pid: Option<Pid>, artifacts: Option<&Artifacts>) -> HelperFacts {
    let (resident, resident_compatible) = resident_facts(pid);
    HelperFacts {
        local_available: Fact::known(artifacts.is_some(), "packaged-helper"),
        local_compatible: Fact::known(artifacts.is_some(), "helper-build-id+protocol"),
        resident,
        resident_compatible,
        attach_without_dialog: Fact::known(true, "bound-c-loader"),
        inspector_thread_set_state: artifacts.map_or_else(
            || Fact::unknown("loader-entitlements", "loader artifact is unavailable"),
            |artifacts| entitlement_fact(&artifacts.loader),
        ),
    }
}

fn resident_facts(pid: Option<Pid>) -> (Fact<bool>, Fact<bool>) {
    let Some(pid) = pid else {
        return (Fact::known(false, "selected-process"), Fact::known(false, "selected-process"));
    };
    let result = (|| -> Result<(bool, Option<bool>), InspectionError> {
        let store = SessionStore::default_root().and_then(SessionStore::open)?;
        let bound = inject::bind_target(pid.0 as i32).map_err(|error| {
            InspectionError::new(InspectionErrorCode::Unavailable, Stage::Attach, error.to_string())
        })?;
        let incarnation = bound.handle.incarnation().into();
        let record = store.live_record_for(incarnation)?;
        let active = record.as_ref().is_some_and(|record| record.state == SessionState::Active);
        let bootstrap = store.endpoint_path(incarnation)?.exists();
        let compatible = if active {
            record.as_ref().map(|record| {
                crate::inspection::reconcile(&store, &record.session_id)
                    .is_ok_and(|state| matches!(state, crate::inspection::Reconciliation::Active))
            })
        } else {
            None
        };
        Ok((active || bootstrap, compatible))
    })();
    match result {
        Ok((resident, compatible)) => (
            Fact::known(resident, "authenticated-session-record+bootstrap-endpoint"),
            compatible.map_or_else(
                || {
                    Fact::unknown(
                        "helper-build-id+protocol",
                        "resident bootstrap compatibility requires an open handshake",
                    )
                },
                |compatible| Fact::known(compatible, "authenticated-reconcile"),
            ),
        ),
        Err(error) => (
            Fact::unknown("authenticated-session-record+bootstrap-endpoint", &error.to_string()),
            Fact::unknown("helper-build-id+protocol", &error.to_string()),
        ),
    }
}

fn entitlement_fact(loader: &Path) -> Fact<bool> {
    let source = "codesign --display --entitlements - --xml recon-loader";
    match std::process::Command::new("codesign")
        .args(["--display", "--entitlements", "-", "--xml"])
        .arg(loader)
        .output()
    {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let entitled =
                text.contains("com.apple.private.thread-set-state") && text.contains("<true/>");
            Fact::known(entitled, source)
        }
        Ok(output) => Fact::unknown(source, &String::from_utf8_lossy(&output.stderr)),
        Err(error) => Fact::unknown(source, &error.to_string()),
    }
}

fn target_selection(application: &Application, pid: Option<Pid>) -> TargetSelection {
    TargetSelection {
        bundle_path: application.path.clone(),
        bundle_id: application.bundle_id.clone(),
        executable_path: application.executable_path.clone(),
        pid: pid.map(|pid| pid.0),
        running_pids: application.running_pids.iter().map(|pid| pid.0).collect(),
    }
}

pub(crate) fn default_store() -> Result<SessionStore, InspectionError> {
    SessionStore::default_root().and_then(SessionStore::open)
}

pub(crate) fn same_bundle(
    record: &crate::inspection::SessionRecord,
    application: &Application,
) -> bool {
    record.bundle_path == application.path
        && application.bundle_id.as_ref().is_some_and(|id| id == &record.bundle_id)
}

fn session_value(
    session: SessionId,
    mut response: Map<String, Value>,
    reused: bool,
) -> Result<Value, InspectionError> {
    response.insert("sessionId".to_owned(), Value::from(session.as_str()));
    response.insert("reused".to_owned(), Value::from(reused));
    Ok(Value::Object(response))
}

fn remove_refused_endpoint(socket: &Path) -> Result<(), InspectionError> {
    if !socket.exists() {
        return Ok(());
    }
    if std::os::unix::net::UnixStream::connect(socket).is_ok() {
        return Ok(());
    }
    fs_remove_private_socket(socket)
}

#[cfg(unix)]
fn fs_remove_private_socket(socket: &Path) -> Result<(), InspectionError> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};

    let metadata = socket.symlink_metadata().map_err(|error| {
        InspectionError::from_io(
            InspectionErrorCode::Unavailable,
            Stage::Connect,
            "the stale helper endpoint could not be inspected",
            &error,
        )
    })?;
    // SAFETY: `geteuid` takes no arguments and only reads this process's effective uid.
    let owner = unsafe { libc::geteuid() };
    if !metadata.file_type().is_socket()
        || metadata.uid() != owner
        || metadata.permissions().mode() & 0o777 != 0o600
    {
        return Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Connect,
            "the existing helper endpoint is not an owner-only socket",
        ));
    }
    std::fs::remove_file(socket).map_err(|error| {
        InspectionError::from_io(
            InspectionErrorCode::Unavailable,
            Stage::Connect,
            "the refused stale helper endpoint could not be removed",
            &error,
        )
    })
}
