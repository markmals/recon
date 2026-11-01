//! Running one `recon ui` command: which instance it observes, what the no-focus guard watches
//! while it runs, and which capability answers it.
//!
//! Nothing here reads a window, an element, or a native object itself. The window server,
//! accessibility, and native-access crates each own their evidence; this module decides what a
//! request selected, keeps the privacy and no-focus boundaries in front of every dispatch, and
//! assembles what they returned into the one JSON object the command prints.

mod correlate;
mod screenshot;
mod session;
mod target;
mod tree;
mod windows;

#[cfg(test)]
mod focus_scope_tests;

use recon_applications_macos::application::Application;
use recon_applications_macos::discovery::with_target;
use recon_core::error::{Failure, InspectionStage};
use recon_core::process::Pid;
use recon_native_access_macos::identity::SessionId;
use recon_privacy_macos::responsibility::{self, ResponsibilityError};
use recon_ui_guard_macos::{Scope, guarded};

use crate::ui::request::{SessionRequest, UiRequest, ViewRequest};
use crate::ui::response::SessionSummary;
use target::{session_record, view_selection, with_instance};

/// Runs one `recon ui` command and returns its result as JSON.
///
/// Recon becomes responsible for its own privacy requests first, then resolves the application,
/// the instance, the output file, and the window, so that a mistaken argument is reported before
/// macOS is ever asked for a permission. Host preparation has no privacy request and bypasses
/// re-execution so the operator can see its interactive confirmation before supplying input.
///
/// # Errors
///
/// Whatever the selected command reports, and the no-focus guard's own observation when running
/// it disturbed the Mac.
pub fn observe(request: &UiRequest) -> Result<String, Failure> {
    if needs_privacy_identity(request) {
        responsibility::take_responsibility().map_err(ResponsibilityError::into_failure)?;
    }
    let scope = scope_of(request)?;
    let selected_pid = scope.target;
    guarded(scope, || dispatch(request, selected_pid))
}

fn needs_privacy_identity(request: &UiRequest) -> bool {
    !matches!(request, UiRequest::Session(SessionRequest::PrepareHost { .. }))
}

/// What the no-focus guard watches while this request runs: the stage a disturbance is reported
/// against, the instance it selected, and whether a setup prompt is an expected interruption.
fn scope_of(request: &UiRequest) -> Result<Scope, Failure> {
    let (stage, selection, setup_prompts_expected) = match request {
        UiRequest::Windows { selection, .. }
        | UiRequest::AxTree { selection, .. }
        | UiRequest::Screenshot { selection, .. }
        | UiRequest::Correlate { selection, .. } => (InspectionStage::Read, Some(selection), false),
        UiRequest::View(view) => (InspectionStage::Read, Some(view_selection(view).0), false),
        UiRequest::Session(SessionRequest::Preflight { selection }) => {
            (InspectionStage::Preflight, Some(selection), false)
        }
        UiRequest::Session(SessionRequest::PrepareHost { .. }) => {
            (InspectionStage::Preflight, None, false)
        }
        UiRequest::Session(SessionRequest::Open { selection, allow_authorization, .. }) => {
            (InspectionStage::Open, Some(selection), *allow_authorization)
        }
        UiRequest::Session(SessionRequest::Close { .. }) => (InspectionStage::Close, None, false),
    };
    let bound_session = match request {
        UiRequest::Windows { session, .. }
        | UiRequest::AxTree { session, .. }
        | UiRequest::Screenshot { session, .. } => session.as_ref(),
        UiRequest::View(view) => Some(view_selection(view).1),
        UiRequest::Correlate { session, .. } => Some(session),
        UiRequest::Session(_) => None,
    };
    // Bind session reads and implicit authorized attaches before starting the no-focus guard.
    let target = match (selection, bound_session) {
        (Some(selected), Some(session)) => Some(with_target(&selected.target, |application| {
            let record = session_record(application, selected.pid, session)?;
            Ok(Pid(record.incarnation.pid as u32))
        })?),
        (Some(selected), None) if setup_prompts_expected && selected.pid.is_none() => {
            Some(with_instance(selected, None, |_, pid| Ok(pid))?)
        }
        (Some(selected), None) => selected.pid,
        (None, _) => None,
    };
    Ok(Scope { stage, target, setup_prompts_expected })
}

fn dispatch(request: &UiRequest, selected_pid: Option<Pid>) -> Result<String, Failure> {
    match request {
        UiRequest::Windows { selection, session } => windows::windows(selection, session.as_ref()),
        UiRequest::AxTree { selection, session, window, depth, no_menu_bar } => {
            tree::tree(selection, session.as_ref(), *window, *depth, *no_menu_bar)
        }
        UiRequest::Screenshot { selection, session, window, out } => {
            screenshot::screenshot(selection, session.as_ref(), *window, out)
        }
        UiRequest::Session(request) => session::session(request, selected_pid),
        UiRequest::View(view) => view_command(view),
        UiRequest::Correlate { selection, session, window, depth, limit } => {
            correlate::correlate(selection, session, *window, *depth, *limit)
        }
    }
}

/// One `recon ui view` command: an action of the AppKit extension, run in the session the
/// request named.
fn view_command(view: &ViewRequest) -> Result<String, Failure> {
    let (selection, session) = view_selection(view);
    with_instance(selection, Some(session), |application, pid| {
        let observation = session::view(application, pid, session, view)?;
        crate::output::encode(&observation)
    })
}

/// The session an external command was bound to, as the summary every response repeats.
///
/// Binding confirms with the helper that still holds the session, so a command that names one
/// reports a stale session rather than observing through a relationship that has ended.
fn bound(
    application: &Application,
    pid: Pid,
    session: Option<&SessionId>,
) -> Result<Option<SessionSummary>, Failure> {
    session
        .map(|session| {
            target::validate_session(application, pid, session).map(|record| SessionSummary {
                id: record.session_id.to_string(),
                acquired: record.established_mode,
                profile: record.profile,
                protocol_version: record.protocol,
                helper_build: record.helper_build.clone(),
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use recon_native_access_macos::request::HostPreparation;

    #[test]
    fn host_preparation_does_not_use_privacy_reexecution() {
        for action in [HostPreparation::Apply, HostPreparation::Status, HostPreparation::Revert] {
            let request = UiRequest::Session(SessionRequest::PrepareHost { action });
            assert!(!needs_privacy_identity(&request), "{action:?} must remain interactive");
        }
    }
}
