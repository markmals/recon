//! Resolving what a `recon ui` request selected: the application, the instance, and the session
//! it is bound to.

use recon_applications_macos::application::Application;
use recon_applications_macos::discovery::with_target;
use recon_applications_macos::selection::{Selection, select_instance};
use recon_core::error::{ErrorCode, Failure};
use recon_core::process::Pid;
use recon_native_access_macos::identity::SessionId;
use recon_native_access_macos::{InspectionError, SessionRecord, appkit, failure};

use crate::ui::request::ViewRequest;
use crate::ui::response::ObservedApplication;

/// The session record this request is bound to, without spending a reconciliation round trip.
pub(super) fn session_record(
    application: &Application,
    requested_pid: Option<Pid>,
    session: &SessionId,
) -> Result<SessionRecord, Failure> {
    appkit::session_record(application, requested_pid, session)
        .map_err(|error| reported(&error, session, requested_pid))
}

/// The session record, confirmed with the helper that still holds it.
pub(super) fn validate_session(
    application: &Application,
    pid: Pid,
    session: &SessionId,
) -> Result<SessionRecord, Failure> {
    appkit::validate_session(application, pid, session)
        .map_err(|error| reported(&error, session, Some(pid)))
}

pub(super) fn reported(error: &InspectionError, session: &SessionId, pid: Option<Pid>) -> Failure {
    failure(error, Some(session), pid)
}

/// The application and instance a selection names, bound through its session when it has one.
pub(super) fn with_instance<T>(
    selection: &Selection,
    session: Option<&SessionId>,
    observe_instance: impl FnOnce(&Application, Pid) -> Result<T, Failure>,
) -> Result<T, Failure> {
    with_target(&selection.target, |application| {
        let pid = match session {
            Some(session) => {
                let record = session_record(application, selection.pid, session)?;
                Pid(record.incarnation.pid as u32)
            }
            None => select_instance(&application.path, &application.running_pids, selection.pid)?,
        };
        observe_instance(application, pid)
    })
}

pub(super) fn observed(application: &Application, pid: Pid) -> ObservedApplication<'_> {
    ObservedApplication { path: &application.path, pid }
}

pub(super) fn view_selection(view: &ViewRequest) -> (&Selection, &SessionId) {
    match view {
        ViewRequest::Tree { selection, session, .. }
        | ViewRequest::Node { selection, session, .. }
        | ViewRequest::Find { selection, session, .. }
        | ViewRequest::Fonts { selection, session, .. } => (selection, session),
    }
}

/// The directory a relative `--out` path is resolved against.
pub(super) fn current_directory() -> Result<std::path::PathBuf, Failure> {
    std::env::current_dir().map_err(|error| {
        Failure::new(
            ErrorCode::IoFailed,
            format!("cannot read the current directory: {error}"),
            serde_json::json!({
                "path": null,
                "operation": "read-current-directory",
                "reason": null,
                "detail": error.to_string(),
                "partialFile": null,
            }),
        )
    })
}
