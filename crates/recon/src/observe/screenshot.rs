//! `recon ui screenshot`: one window's pixels, written to a file that did not exist.

use std::path::Path;

use recon_accessibility_macos::element;
use recon_accessibility_macos::window_element;
use recon_applications_macos::application::Application;
use recon_applications_macos::selection::Selection;
use recon_core::error::{ErrorCode, Failure, NotFound};
use recon_core::process::Pid;
use recon_native_access_macos::SessionId;
use recon_privacy_macos::permission::Permission;
use recon_privacy_macos::permissions;
use recon_window_server_macos::capture;
use recon_window_server_macos::inventory::windows_owned_by;
use recon_window_server_macos::window::{Window, WindowId, select_window};

use super::bound;
use super::target::{current_directory, observed, with_instance};
use crate::ui::output::{prepare_output, write_new};
use crate::ui::response::{CapturedImage, CapturedWindow, ScreenshotResponse};

pub(super) fn screenshot(
    selection: &Selection,
    session: Option<&SessionId>,
    window: Option<WindowId>,
    out: &Path,
) -> Result<String, Failure> {
    with_instance(selection, session, |application, pid| {
        let summary = bound(application, pid, session)?;
        let out = prepare_output(out, &current_directory()?)?;
        let window = match window {
            Some(id) => {
                let window = select_window(windows_owned_by(pid)?, id, pid)?;
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
        let response = match summary {
            Some(summary) => response.with_session(summary),
            None => response,
        };
        let response = crate::output::encode(&response)?;
        write_new(&out, &captured.png)?;
        Ok(response)
    })
}

/// The window the application itself reports as its main one, as the window server lists it.
///
/// # Errors
///
/// [`ErrorCode::NotFound`] with the kind `main-window` when the application reports none.
fn main_window(application: &Application, pid: Pid) -> Result<Window, Failure> {
    element::bound_messaging();
    let application_element = element::application_element(pid)?;
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
    select_window(windows_owned_by(pid)?, id, pid)
}
