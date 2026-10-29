//! Observing one running instance of a macOS application: its windows, its accessibility tree,
//! and the pixels of one window.

mod accessibility;
mod attributes;
mod capture;
mod child_output;
pub(super) mod context;
mod correlate;
mod focus;
mod forwarding;
mod inspection;
mod permissions;
mod private;
mod responsibility;
mod signing;
mod window_element;
mod window_server;

#[cfg(test)]
mod menu_bar_tests;

use std::env;
use std::path::{Path, PathBuf};

use accessibility::{Accessibility, Element};
use serde_json::json;

use super::encode;
use crate::application::{Application, Pid};
use crate::error::InspectionStage;
use crate::error::{ErrorCode, Failure, NotFound};
use crate::inspection::{AccessProfile, AcquisitionMode, SessionId, SessionRecord};
use crate::ui::instance::select_instance;
use crate::ui::output::{prepare_output, write_new};
use crate::ui::permission::Permission;
use crate::ui::request::{
    AccessMode, AccessProfile as UiAccessProfile, FieldSet, NativeScope, Selection, SessionRequest,
    TraversalKind, UiRequest, ViewRequest,
};
use crate::ui::response::{
    CapturedImage, CapturedWindow, ObservedApplication, ScreenshotResponse, SessionSummary,
    TreeResponse, WindowsResponse,
};
use crate::ui::tree::{TreeOptions, read_tree};
use crate::ui::window::{Window, WindowId, select_window};

/// Runs one `recon ui` command and returns its result as JSON.
///
/// Recon becomes responsible for its own privacy requests first, then resolves the application,
/// the instance, the output file, and the window, so that a mistaken argument is reported before
/// macOS is ever asked for a permission.
pub fn observe(request: &UiRequest) -> Result<String, Failure> {
    responsibility::take_responsibility()
        .map_err(responsibility::ResponsibilityError::into_failure)?;
    focus::guarded(scope_of(request), || dispatch(request))
}

/// What the no-focus guard watches while this request runs: the stage a disturbance is reported
/// against, the instance it selected, and whether a setup prompt is an expected interruption.
fn scope_of(request: &UiRequest) -> focus::Scope {
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
    focus::Scope {
        stage,
        target: selection.and_then(|selection| selection.pid),
        setup_prompts_expected,
    }
}

fn dispatch(request: &UiRequest) -> Result<String, Failure> {
    match request {
        UiRequest::Windows { selection, session } => windows(selection, session.as_ref()),
        UiRequest::AxTree { selection, session, window, depth, no_menu_bar } => {
            tree(selection, session.as_ref(), *window, *depth, *no_menu_bar)
        }
        UiRequest::Screenshot { selection, session, window, out } => {
            screenshot(selection, session.as_ref(), *window, out)
        }
        UiRequest::Session(SessionRequest::Preflight { selection }) => {
            super::with_target(&selection.target, |application| {
                encode_inspection(
                    inspection::preflight(application, selection.pid),
                    None,
                    selection.pid,
                )
            })
        }
        UiRequest::Session(SessionRequest::PrepareHost { action }) => {
            encode_inspection(inspection::prepare_host(*action), None, None)
        }
        UiRequest::Session(SessionRequest::Open {
            selection,
            mode,
            profile,
            allow_authorization,
        }) => super::with_target(&selection.target, |application| {
            encode_inspection(
                inspection::open_session(
                    application,
                    selection,
                    acquisition_mode(*mode),
                    access_profile(*profile),
                    *allow_authorization,
                ),
                None,
                selection.pid,
            )
        }),
        UiRequest::Session(SessionRequest::Close { session }) => {
            encode_inspection(inspection::close_session(session), Some(session), None)
        }
        UiRequest::View(view) => {
            let (selection, session) = view_selection(view);
            with_instance(selection, |application, pid| {
                encode_inspection(
                    inspection::view(application, pid, view),
                    Some(session),
                    Some(pid),
                )
            })
        }
        UiRequest::Correlate { selection, session, window, depth, limit } => {
            correlate(selection, session, *window, *depth, *limit)
        }
    }
}

fn windows(selection: &Selection, session: Option<&SessionId>) -> Result<String, Failure> {
    with_instance(selection, |application, pid| {
        permissions::require(&[Permission::ScreenRecording])?;
        let windows = window_server::windows_owned_by(pid)?;
        let mut response =
            serde_json::to_value(WindowsResponse::new(observed(application, pid), windows))
                .map_err(|error| Failure::internal(error.to_string()))?;
        if let Some(session) = session {
            let record = inspection::validate_session(application, pid, session)
                .map_err(|error| inspection::failure(&error, Some(session), Some(pid)))?;
            let native = inspection::native_windows(application, pid, session)
                .map_err(|error| inspection::failure(&error, Some(session), Some(pid)))?;
            response["session"] = serde_json::to_value(session_summary(&record))
                .map_err(|error| Failure::internal(error.to_string()))?;
            response["nativeWindows"] = native["windows"].clone();
            response["coverage"] = native["coverage"].clone();
            response["observation"] = native["observation"].clone();
        }
        super::encode(&response)
    })
}

fn tree(
    selection: &Selection,
    session: Option<&SessionId>,
    window: Option<WindowId>,
    depth: Option<usize>,
    no_menu_bar: bool,
) -> Result<String, Failure> {
    with_instance(selection, |application, pid| {
        let record = session
            .map(|session| inspection::validate_session(application, pid, session))
            .transpose()
            .map_err(|error| inspection::failure(&error, session, Some(pid)))?;
        if let Some(id) = window {
            select_window(window_server::windows_owned_by(pid)?, id, pid)?;
        }
        permissions::require(&[Permission::Accessibility])?;

        accessibility::bound_messaging();
        let application_element = accessibility::application_element(pid)?;
        let excluded_root_child = if no_menu_bar && depth != Some(0) {
            accessibility::menu_bar(&application_element, pid)?
        } else {
            None
        };
        let root = match window {
            Some(id) => window_element::window_element(&application_element, pid, id)?,
            None => application_element,
        };

        let tree = read_tree(
            &mut Accessibility::new(),
            Element::new(root),
            TreeOptions { depth, excluded_root_child },
        );
        let response = TreeResponse::new(
            observed(application, pid),
            window,
            tree.truncated,
            tree.narrowed,
            tree.root,
        );
        let response = match record {
            Some(record) => response.with_session(session_summary(&record)),
            None => response,
        };
        encode(&response)
    })
}

fn screenshot(
    selection: &Selection,
    session: Option<&SessionId>,
    window: Option<WindowId>,
    out: &Path,
) -> Result<String, Failure> {
    with_instance(selection, |application, pid| {
        let record = session
            .map(|session| inspection::validate_session(application, pid, session))
            .transpose()
            .map_err(|error| inspection::failure(&error, session, Some(pid)))?;
        let out = prepare_output(out, &current_directory()?)?;
        let window = match window {
            Some(id) => {
                let window = select_window(window_server::windows_owned_by(pid)?, id, pid)?;
                permissions::require(&[Permission::ScreenRecording])?;
                window
            }
            None => {
                permissions::require(&[Permission::ScreenRecording, Permission::Accessibility])?;
                main_window(application, pid)?
            }
        };

        let captured = capture::capture(window.id)?;
        // Encoding first keeps the promise that a failed command writes no file.
        let response = ScreenshotResponse::new(
            observed(application, pid),
            CapturedWindow {
                id: window.id,
                title: window.title.as_deref(),
                bounds: window.bounds,
                on_screen: window.on_screen,
            },
            CapturedImage {
                path: &out,
                width: captured.width,
                height: captured.height,
                uniform: captured.uniform,
            },
        );
        let response = match record {
            Some(record) => response.with_session(session_summary(&record)),
            None => response,
        };
        let response = encode(&response)?;
        write_new(&out, &captured.png)?;
        Ok(response)
    })
}

fn correlate(
    selection: &Selection,
    session: &SessionId,
    window: WindowId,
    depth: usize,
    limit: usize,
) -> Result<String, Failure> {
    let native = with_instance(selection, |application, pid| {
        let request = ViewRequest::Tree {
            selection: selection.clone(),
            session: session.clone(),
            scope: NativeScope { window: Some(window), root: None },
            kind: TraversalKind::View,
            depth,
            fields: FieldSet::default(),
            limit,
        };
        inspection::view(application, pid, &request)
            .map_err(|error| inspection::failure(&error, Some(session), Some(pid)))
    })?;
    let accessibility = tree(selection, Some(session), Some(window), Some(depth), false)?;
    let accessibility: serde_json::Value = serde_json::from_str(&accessibility)
        .map_err(|error| Failure::internal(format!("cannot decode AX observation: {error}")))?;
    let correlation = correlate::correlate(&native, &accessibility);
    let candidates = correlation
        .candidates
        .iter()
        .map(|candidate| {
            serde_json::json!({
                "native": native_reference(&candidate.native),
                "accessibility": element_reference(&candidate.accessibility),
                "basis": match candidate.basis {
                    correlate::MatchBasis::AccessibilityIdentifier => "accessibility-identifier",
                    correlate::MatchBasis::RoleAndBounds => "role-and-bounds",
                },
                "ambiguous": candidate.ambiguous,
            })
        })
        .collect::<Vec<_>>();
    encode(&serde_json::json!({
        "native": native,
        "accessibility": accessibility,
        "candidates": candidates,
        "unmatchedNative": correlation.unmatched_native.iter().map(native_reference).collect::<Vec<_>>(),
        "unmatchedAccessibility": correlation.unmatched_accessibility.iter().map(element_reference).collect::<Vec<_>>(),
    }))
}

fn native_reference(reference: &correlate::NativeReference) -> serde_json::Value {
    serde_json::json!({
        "handle": reference.handle,
        "kind": reference.kind,
        "class": reference.class,
    })
}

fn element_reference(reference: &correlate::ElementReference) -> serde_json::Value {
    serde_json::json!({
        "path": reference.path,
        "role": reference.role,
        "identifier": reference.identifier,
    })
}

fn acquisition_mode(mode: AccessMode) -> AcquisitionMode {
    match mode {
        AccessMode::Existing => AcquisitionMode::Existing,
        AccessMode::Launch => AcquisitionMode::Launch,
        AccessMode::Attach => AcquisitionMode::Attach,
    }
}

fn access_profile(profile: UiAccessProfile) -> AccessProfile {
    match profile {
        UiAccessProfile::Standard => AccessProfile::Standard,
        UiAccessProfile::Research => AccessProfile::Research,
    }
}

fn view_selection(view: &ViewRequest) -> (&Selection, &SessionId) {
    match view {
        ViewRequest::Tree { selection, session, .. }
        | ViewRequest::Node { selection, session, .. }
        | ViewRequest::Find { selection, session, .. }
        | ViewRequest::Fonts { selection, session, .. } => (selection, session),
    }
}

fn encode_inspection(
    result: Result<serde_json::Value, crate::inspection::InspectionError>,
    session: Option<&SessionId>,
    pid: Option<Pid>,
) -> Result<String, Failure> {
    let value = result.map_err(|error| inspection::failure(&error, session, pid))?;
    super::encode(&value)
}

fn session_summary(record: &SessionRecord) -> SessionSummary {
    SessionSummary {
        id: record.session_id.to_string(),
        acquired: match record.established_mode {
            AcquisitionMode::Existing => AccessMode::Existing,
            AcquisitionMode::Launch => AccessMode::Launch,
            AcquisitionMode::Attach => AccessMode::Attach,
        },
        profile: match record.profile {
            AccessProfile::Standard => UiAccessProfile::Standard,
            AccessProfile::Research => UiAccessProfile::Research,
        },
        protocol_version: record.protocol,
        helper_build: record.helper_build.clone(),
    }
}

/// The window the application itself reports as its main one, as the window server lists it.
///
/// # Errors
///
/// [`ErrorCode::NotFound`] with the kind `main-window` when the application reports none.
fn main_window(application: &Application, pid: Pid) -> Result<Window, Failure> {
    accessibility::bound_messaging();
    let application_element = accessibility::application_element(pid)?;
    let id = window_element::main_window_id(&application_element, pid)?.ok_or_else(|| {
        Failure::new(
            ErrorCode::NotFound,
            format!(
                "{} reports no main window; choose a window from recon ui windows and pass \
                 --window",
                application.path.display()
            ),
            NotFound {
                kind: "main-window",
                path: Some(application.path.display().to_string()),
                pid: Some(pid.0),
                ..Default::default()
            }
            .details(),
        )
    })?;
    select_window(window_server::windows_owned_by(pid)?, id, pid)
}

fn with_instance<T>(
    selection: &Selection,
    observe_instance: impl FnOnce(&Application, Pid) -> Result<T, Failure>,
) -> Result<T, Failure> {
    super::with_target(&selection.target, |application| {
        let pid = select_instance(&application.path, &application.running_pids, selection.pid)?;
        observe_instance(application, pid)
    })
}

fn observed(application: &Application, pid: Pid) -> ObservedApplication<'_> {
    ObservedApplication { path: &application.path, pid }
}

fn current_directory() -> Result<PathBuf, Failure> {
    env::current_dir().map_err(|error| {
        Failure::new(
            ErrorCode::IoFailed,
            format!("cannot read the current directory: {error}"),
            json!({
                "path": null,
                "operation": "read-current-directory",
                "reason": null,
                "detail": error.to_string(),
                "partialFile": null,
            }),
        )
    })
}
