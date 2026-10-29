//! Session establishment against an exact application and process incarnation.

use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::artifact::{self, Artifacts};
use crate::application::{Application, Pid};
use crate::error::ErrorCode;
use crate::inspection::{
    AccessProfile, AcquisitionMode, CloseOutcome, Incarnation, InspectionError,
    InspectionErrorCode, LIFECYCLE_DEADLINE, OpenRequest, SessionId, SessionState, SessionStore,
    Stage, close_until, open_until,
};
use crate::macos::inject;
use crate::macos::inspection_access::{
    self, AccessMode, CommandRunner, Eligibility, Fact, HelperFacts, ProfileHelperFacts,
    SystemRunner, TargetSelection,
};
use crate::macos::workspace;
use crate::ui::request::Selection;

pub(crate) fn preflight(
    application: &Application,
    pid: Option<Pid>,
) -> Result<Value, InspectionError> {
    let pid =
        pid.or_else(|| (application.running_pids.len() == 1).then(|| application.running_pids[0]));
    let standard = artifact::locate(AccessProfile::Standard).ok();
    let research = artifact::locate(AccessProfile::Research).ok();
    let resident = resident_facts(pid);
    let helper = profile_helpers(standard.as_ref(), research.as_ref(), &resident);
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
    expected_pid: Option<Pid>,
    mode: AcquisitionMode,
    profile: AccessProfile,
    allow_authorization: bool,
) -> Result<Value, InspectionError> {
    let deadline = Instant::now() + LIFECYCLE_DEADLINE;
    let artifacts = artifact::locate(profile);
    if mode == AcquisitionMode::Launch && !application.running_pids.is_empty() {
        return Err(InspectionError::new(
            InspectionErrorCode::Busy,
            Stage::Attach,
            "launch mode refuses an already-running application; use existing or attach",
        ));
    }
    if mode == AcquisitionMode::Launch && expected_pid.is_some() {
        return Err(InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Attach,
            "the guarded process selection cannot be used to launch another process",
        ));
    }
    let pid = match mode {
        AcquisitionMode::Launch => {
            check_access(
                application,
                None,
                mode,
                profile,
                allow_authorization,
                artifacts.as_ref().map_err(Clone::clone)?,
            )?;
            workspace::launch_background(&application.path, deadline).map_err(|error| {
                let timed_out = error.code == ErrorCode::InspectionTimeout;
                let failure = InspectionError::new(
                    if timed_out {
                        InspectionErrorCode::Timeout
                    } else {
                        InspectionErrorCode::Unavailable
                    },
                    Stage::Attach,
                    format!("background application launch failed: {error}"),
                );
                if timed_out { failure.target_execution_may_continue() } else { failure }
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
            if expected_pid.is_some_and(|expected| expected != pid) {
                return Err(InspectionError::new(
                    InspectionErrorCode::StaleSession,
                    Stage::Attach,
                    "the selected process changed after the no-focus guard bound its PID",
                ));
            }
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
    let executable = selected_executable(application, pid)?;
    let store = default_store()?;
    let _lifecycle = store.lock_lifecycle(incarnation)?;
    let pending = store.attach_reservation(incarnation)?;
    if pending.as_ref().is_some_and(|reserved| reserved != &executable) {
        return Err(InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Attach,
            "the unfinished attach names a different executable for this process incarnation",
        )
        .target_execution_may_continue());
    }
    let artifacts = artifacts.map_err(|error| {
        if pending.is_some() {
            InspectionError::new(
                InspectionErrorCode::Busy,
                Stage::Attach,
                format!(
                    "the unfinished attach to {incarnation} cannot be authenticated without \
                     matching local helper artifacts; Recon will not load another helper"
                ),
            )
            .target_execution_may_continue()
        } else {
            error
        }
    })?;
    let recorded = store.live_record_for(incarnation)?;
    if let Some(record) = &recorded
        && !same_bundle(record, application)
    {
        return Err(InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Handshake,
            "the selected session belongs to a different bundle or bundle location",
        ));
    }
    // An existing record is reconciled by open_until within the same deadline. Running the
    // read-only eligibility probe first would reconcile it with a fresh ten-second budget.
    if mode != AcquisitionMode::Launch && pending.is_none() && recorded.is_none() {
        check_access(application, Some(pid), mode, profile, allow_authorization, &artifacts)?;
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
        match open_until(&store, &request, deadline) {
            Ok(outcome) => {
                if pending.is_some() {
                    store.complete_attach(incarnation)?;
                }
                return session_value(outcome.record.session_id, outcome.response, outcome.reused);
            }
            // Only attach may fall through to loading a helper, and only when there is no recorded
            // session to resolve first. A connection failure to a session Recon still has on record
            // means that session is unresolved, not absent: reinjecting would risk a second helper
            // while existing access is unaccounted for, so surface the error instead.
            Err(error) if pending.is_some() && error.stage() == Stage::Connect => {
                if store.clear_exited_attach(incarnation)? {
                    return Err(InspectionError::new(
                        InspectionErrorCode::StaleSession,
                        Stage::Peer,
                        "the unfinished attach's process has exited",
                    ));
                }
                return Err(uncertain_attach(incarnation));
            }
            Err(error) if pending.is_some() => {
                return Err(error.target_execution_may_continue());
            }
            Err(error)
                if mode == AcquisitionMode::Attach
                    && error.stage() == Stage::Connect
                    && store.live_record_for(incarnation)?.is_none() => {}
            Err(error) => return Err(error),
        }
    }

    if Instant::now() >= deadline {
        return Err(InspectionError::new(
            InspectionErrorCode::Timeout,
            Stage::Attach,
            "the open deadline expired before the helper loader started",
        ));
    }
    remove_refused_endpoint(&socket)?;
    store.reserve_attach(incarnation, &executable)?;
    artifact::load(&artifacts, incarnation, &socket, &executable, deadline)
        .map_err(InspectionError::target_execution_may_continue)?;
    loop {
        match open_until(&store, &request, deadline) {
            Ok(outcome) => {
                store.complete_attach(incarnation)?;
                return session_value(outcome.record.session_id, outcome.response, outcome.reused);
            }
            Err(error) if error.stage() == Stage::Connect && Instant::now() < deadline => {
                sleep(Duration::from_millis(20));
            }
            Err(error) if error.stage() == Stage::Connect => {
                if store.clear_exited_attach(incarnation)? {
                    return Err(InspectionError::new(
                        InspectionErrorCode::StaleSession,
                        Stage::Peer,
                        "the selected process exited before the helper answered",
                    ));
                }
                return Err(InspectionError::new(
                    InspectionErrorCode::Timeout,
                    Stage::Connect,
                    "the open deadline expired before the helper answered; the target may still be starting its bootstrap",
                )
                .target_execution_may_continue());
            }
            Err(error) => return Err(error.target_execution_may_continue()),
        }
    }
}

fn uncertain_attach(incarnation: Incarnation) -> InspectionError {
    InspectionError::new(
        InspectionErrorCode::Busy,
        Stage::Attach,
        format!(
            "the helper for {incarnation} is unreachable after an unfinished attach; Recon will \
             not remove its endpoint or load another helper until authenticated reconciliation \
             or verified process exit"
        ),
    )
    .target_execution_may_continue()
}

pub(crate) fn close_session(session: &SessionId) -> Result<Value, InspectionError> {
    let deadline = Instant::now() + LIFECYCLE_DEADLINE;
    let store = default_store()?;
    let record = store.record(session)?.ok_or_else(|| {
        InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Store,
            format!("session {session} is not recorded for this user"),
        )
    })?;
    let _lifecycle = store.lock_lifecycle(record.incarnation)?;
    let outcome = close_until(&store, session, deadline)?;
    let (state, acknowledged, process_exited) = match outcome {
        CloseOutcome::Acknowledged { was_active } => {
            (if was_active { "closed" } else { "already-closed" }, true, false)
        }
        CloseOutcome::AlreadyClosed => ("already-closed", true, false),
        CloseOutcome::ReconciledClosed { process_exited } => {
            ("closed-after-reconciliation", true, process_exited)
        }
        CloseOutcome::Unconfirmed(error) => {
            let failure = InspectionError::new(
                error.code(),
                Stage::Close,
                format!(
                    "session {session} close-unconfirmed: {error}; access may remain active until an authenticated close or process exit"
                ),
            );
            return Err(if error.may_continue_in_target() {
                failure.target_execution_may_continue()
            } else {
                failure
            });
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

fn selected_executable(application: &Application, pid: Pid) -> Result<PathBuf, InspectionError> {
    let mut runner = SystemRunner;
    let observed = runner.process_executable(pid.0).map_err(|error| {
        InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Attach,
            format!("the selected process executable could not be read: {error}"),
        )
    })?;
    let observed = std::fs::canonicalize(&observed).unwrap_or_else(|_| PathBuf::from(&observed));
    if let Some(expected) = &application.executable_path {
        let expected = std::fs::canonicalize(expected).unwrap_or_else(|_| expected.clone());
        if expected != observed {
            return Err(InspectionError::new(
                InspectionErrorCode::StaleSession,
                Stage::Attach,
                "the selected process is no longer running the selected bundle's executable",
            ));
        }
    } else if !observed.starts_with(&application.path) {
        return Err(InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Attach,
            "the selected process executable is outside the selected bundle",
        ));
    }
    Ok(observed)
}

fn check_access(
    application: &Application,
    pid: Option<Pid>,
    mode: AcquisitionMode,
    profile: AccessProfile,
    allow_authorization: bool,
    artifacts: &Artifacts,
) -> Result<(), InspectionError> {
    let resident = resident_facts(pid);
    let helper = helper_facts(Some(artifacts), profile, &resident);
    let target = target_selection(application, pid);
    let facts = inspection_access::collect_preflight(
        &mut SystemRunner,
        target,
        ProfileHelperFacts { standard: helper.clone(), research: helper },
    );
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

enum ResidentObservation {
    Absent,
    Unverified(String),
    Active { profile: AccessProfile, helper_build: String, protocol: u32 },
}

fn profile_helpers(
    standard: Option<&Artifacts>,
    research: Option<&Artifacts>,
    observed: &ResidentObservation,
) -> ProfileHelperFacts {
    ProfileHelperFacts {
        standard: helper_facts(standard, AccessProfile::Standard, observed),
        research: helper_facts(research, AccessProfile::Research, observed),
    }
}

fn helper_facts(
    artifacts: Option<&Artifacts>,
    profile: AccessProfile,
    observed: &ResidentObservation,
) -> HelperFacts {
    let candidate = artifacts.map_or_else(
        || format!("no matching {} packaged helper and loader", profile),
        |artifacts| {
            format!(
                "{} with {} (build {})",
                artifacts.helper.display(),
                artifacts.loader.display(),
                artifacts.helper_build
            )
        },
    );
    let (resident, resident_compatible) = match observed {
        ResidentObservation::Absent => (
            Fact::known(false, "selected-process+bootstrap-endpoint"),
            Fact::known(false, "selected-process+bootstrap-endpoint"),
        ),
        ResidentObservation::Unverified(reason) => {
            (Fact::unknown("bootstrap-endpoint", reason), Fact::unknown(&candidate, reason))
        }
        ResidentObservation::Active { profile: active_profile, helper_build, protocol } => (
            Fact::known(true, "authenticated-reconcile"),
            artifacts.map_or_else(
                || Fact::unknown(&candidate, "matching local build is unavailable"),
                |artifacts| {
                    Fact::known(
                        *active_profile == profile
                            && helper_build == &artifacts.helper_build
                            && *protocol == 1,
                        &format!("authenticated-reconcile; {candidate}"),
                    )
                },
            ),
        ),
    };
    HelperFacts {
        local_available: Fact::known(artifacts.is_some(), &candidate),
        local_compatible: Fact::unknown(
            &candidate,
            "the packaged build identity does not prove target-side protocol compatibility",
        ),
        resident,
        resident_compatible,
        attach_without_dialog: Fact::known(true, "bound-c-loader"),
        inspector_thread_set_state: artifacts.map_or_else(
            || Fact::unknown("loader-entitlements", "loader artifact is unavailable"),
            |artifacts| entitlement_fact(&artifacts.loader),
        ),
    }
}

fn resident_facts(pid: Option<Pid>) -> ResidentObservation {
    let Some(pid) = pid else {
        return ResidentObservation::Absent;
    };
    let result = (|| -> Result<ResidentObservation, InspectionError> {
        let store = SessionStore::default_root().and_then(SessionStore::open)?;
        let bound = inject::bind_target(pid.0 as i32).map_err(|error| {
            InspectionError::new(InspectionErrorCode::Unavailable, Stage::Attach, error.to_string())
        })?;
        let incarnation = bound.handle.incarnation().into();
        let record = store.live_record_for(incarnation)?;
        let bootstrap = store.endpoint_path(incarnation)?.exists();
        if let Some(record) = record.filter(|record| record.state == SessionState::Active)
            && matches!(
                crate::inspection::reconcile(&store, &record.session_id)?,
                crate::inspection::Reconciliation::Active
            )
        {
            return Ok(ResidentObservation::Active {
                profile: record.profile,
                helper_build: record.helper_build,
                protocol: record.protocol,
            });
        }
        if bootstrap {
            Ok(ResidentObservation::Unverified(
                "an endpoint exists, but residency and compatibility require authenticated negotiation"
                    .into(),
            ))
        } else {
            Ok(ResidentObservation::Absent)
        }
    })();
    result.unwrap_or_else(|error| ResidentObservation::Unverified(error.to_string()))
}

fn entitlement_fact(loader: &Path) -> Fact<bool> {
    let source = format!("codesign --display --entitlements - --xml {}", loader.display());
    match std::process::Command::new("codesign")
        .args(["--display", "--entitlements", "-", "--xml"])
        .arg(loader)
        .output()
    {
        Ok(output) if output.status.success() => {
            parse_thread_state_entitlement(&output.stdout, &source)
        }
        Ok(output) => Fact::unknown(&source, &String::from_utf8_lossy(&output.stderr)),
        Err(error) => Fact::unknown(&source, &error.to_string()),
    }
}

fn parse_thread_state_entitlement(xml: &[u8], source: &str) -> Fact<bool> {
    if xml.is_empty() {
        return Fact::known(false, source);
    }
    match plist::Value::from_reader_xml(xml) {
        Ok(plist::Value::Dictionary(entitlements)) => {
            match entitlements.get("com.apple.private.thread-set-state") {
                None => Fact::known(false, source),
                Some(value) => value
                    .as_boolean()
                    .map(|enabled| Fact::known(enabled, source))
                    .unwrap_or_else(|| Fact::unknown(source, "entitlement is not a Boolean")),
            }
        }
        _ => Fact::unknown(source, "entitlements are not a readable XML dictionary"),
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

#[cfg(test)]
mod preflight_tests {
    use super::*;

    #[test]
    fn proposal0005_synthetic_profile_candidates_do_not_prove_local_compatibility() {
        let root = tempfile::tempdir().unwrap();
        let standard = root.path().join("standard");
        let research = root.path().join("research");
        std::fs::create_dir_all(&standard).unwrap();
        std::fs::create_dir_all(&research).unwrap();
        let candidate = |directory: &Path| {
            let helper = directory.join("recon-helper.dylib");
            let loader = directory.join("recon-loader");
            std::fs::write(&helper, b"not a helper").unwrap();
            std::fs::write(&loader, b"not a loader").unwrap();
            Artifacts { helper, loader, helper_build: "0".repeat(64) }
        };
        let standard = candidate(&standard);
        let research = candidate(&research);
        let rows = profile_helpers(Some(&standard), Some(&research), &ResidentObservation::Absent);

        assert_eq!(rows.standard.local_available.value, Some(true));
        assert_eq!(rows.research.local_available.value, Some(true));
        assert_eq!(rows.standard.local_compatible.value, None);
        assert_eq!(rows.research.local_compatible.value, None);
        assert!(
            rows.standard.local_available.source.contains(&standard.helper.display().to_string()),
            "standard facts must identify the standard candidate"
        );
        assert!(
            rows.research.local_available.source.contains(&research.helper.display().to_string()),
            "research facts must identify the research candidate"
        );
        assert!(
            rows.standard
                .inspector_thread_set_state
                .source
                .contains(&standard.loader.display().to_string()),
            "standard signing facts must identify the selected loader"
        );
        assert!(
            rows.research
                .inspector_thread_set_state
                .source
                .contains(&research.loader.display().to_string()),
            "research signing facts must identify the selected loader"
        );
    }

    #[test]
    fn proposal0005_resident_helper_matches_both_profile_and_selected_build() {
        let root = tempfile::tempdir().unwrap();
        let candidate = |name: &str, build: &str| Artifacts {
            helper: root.path().join(name).join("recon-helper.dylib"),
            loader: root.path().join(name).join("recon-loader"),
            helper_build: build.to_owned(),
        };
        let standard = candidate("standard", "1");
        let research = candidate("research", "2");
        let observed = ResidentObservation::Active {
            profile: AccessProfile::Research,
            helper_build: "2".into(),
            protocol: 1,
        };
        assert_eq!(
            helper_facts(Some(&standard), AccessProfile::Standard, &observed)
                .resident_compatible
                .value,
            Some(false)
        );
        assert_eq!(
            helper_facts(Some(&research), AccessProfile::Research, &observed)
                .resident_compatible
                .value,
            Some(true)
        );
        let other_build = candidate("research", "3");
        assert_eq!(
            helper_facts(Some(&other_build), AccessProfile::Research, &observed)
                .resident_compatible
                .value,
            Some(false)
        );
    }

    #[test]
    fn proposal0005_loader_entitlement_requires_the_selected_boolean_key() {
        let source = "synthetic research loader entitlements";
        let false_key = br#"<?xml version="1.0"?><plist version="1.0"><dict><key>com.apple.private.thread-set-state</key><false/><key>other</key><true/></dict></plist>"#;
        let true_key = br#"<?xml version="1.0"?><plist version="1.0"><dict><key>com.apple.private.thread-set-state</key><true/></dict></plist>"#;
        assert_eq!(parse_thread_state_entitlement(false_key, source).value, Some(false));
        assert_eq!(parse_thread_state_entitlement(true_key, source).value, Some(true));
        assert_eq!(parse_thread_state_entitlement(b"not a plist", source).value, None);
    }
}
