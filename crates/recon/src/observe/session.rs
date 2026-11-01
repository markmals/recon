//! `recon ui session` and `recon ui view`: the inspection-access lifecycle, and the AppKit
//! actions that run inside an open session.

use recon_applications_macos::application::Application;
use recon_applications_macos::selection::select_instance;
use recon_core::action::{ActionId, Invocation, invoke};
use recon_core::error::Failure;
use recon_core::process::Pid;
use recon_native_access_macos::appkit::{AppKit, ViewQuery};
use recon_native_access_macos::identity::SessionId;
use recon_native_access_macos::{InspectionError, close_session, failure, open_session};
use serde_json::Value;

use crate::cli::registry;
use crate::ui::request::{SessionRequest, ViewRequest};

pub(super) fn session(
    request: &SessionRequest,
    selected_pid: Option<Pid>,
) -> Result<String, Failure> {
    match request {
        SessionRequest::Preflight { selection } => {
            recon_applications_macos::discovery::with_target(&selection.target, |application| {
                let pid = if application.running_pids.is_empty() && selection.pid.is_none() {
                    None
                } else {
                    Some(select_instance(
                        &application.path,
                        &application.running_pids,
                        selection.pid,
                    )?)
                };
                encode(recon_native_access_macos::preflight(application, pid), None, pid)
            })
        }
        SessionRequest::PrepareHost { action } => {
            encode(recon_native_access_macos::prepare_host(*action), None, None)
        }
        SessionRequest::Open { selection, mode, profile, allow_authorization } => {
            recon_applications_macos::discovery::with_target(&selection.target, |application| {
                encode(
                    open_session(
                        application,
                        selection,
                        selected_pid,
                        *mode,
                        *profile,
                        *allow_authorization,
                    ),
                    None,
                    selection.pid,
                )
            })
        }
        SessionRequest::Close { session } => encode(close_session(session), Some(session), None),
    }
}

/// Runs the AppKit action one `ui view` command is mapped to, in the session it named.
pub(super) fn view(
    application: &Application,
    pid: Pid,
    session: &SessionId,
    request: &ViewRequest,
) -> Result<Value, Failure> {
    let extension = AppKit::new(application, pid, session);
    let invocation = Invocation { action: action_of(request), input: query(request).input()? };
    invoke(&extension, invocation).map(Value::Object)
}

/// The action the command registry maps this `ui view` command to.
///
/// The registry is the one place a command's grammar, help, schema, and action identity are
/// declared together, so the dispatch asks it rather than keeping a second mapping.
fn action_of(request: &ViewRequest) -> ActionId {
    let path: &[&str] = match request {
        ViewRequest::Tree { .. } => &["ui", "view", "tree"],
        ViewRequest::Node { .. } => &["ui", "view", "node"],
        ViewRequest::Find { .. } => &["ui", "view", "find"],
        ViewRequest::Fonts { .. } => &["ui", "view", "fonts"],
    };
    registry::action(path).expect("every ui view command declares the action it runs")
}

fn query(request: &ViewRequest) -> ViewQuery<'_> {
    match request {
        ViewRequest::Tree { scope, kind, depth, fields, limit, .. } => {
            ViewQuery::Tree { scope, kind: *kind, depth: *depth, fields, limit: *limit }
        }
        ViewRequest::Node { handle, fields, .. } => ViewQuery::Node { handle, fields },
        ViewRequest::Find { scope, kind, filters, depth, fields, limit, .. } => {
            ViewQuery::Find { scope, kind: *kind, filters, depth: *depth, fields, limit: *limit }
        }
        ViewRequest::Fonts { scope, depth, limit, .. } => {
            ViewQuery::Fonts { scope, depth: *depth, limit: *limit }
        }
    }
}

fn encode(
    result: Result<Value, InspectionError>,
    session: Option<&SessionId>,
    pid: Option<Pid>,
) -> Result<String, Failure> {
    let value = result.map_err(|error| failure(&error, session, pid))?;
    crate::output::encode(&value)
}
