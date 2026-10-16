//! Observing one running instance of a macOS application: its windows, its accessibility tree,
//! and the pixels of one window.

mod accessibility;
mod attributes;
mod capture;
mod permissions;
mod private;
mod responsibility;
mod window_element;
mod window_server;

use std::env;
use std::path::{Path, PathBuf};

use serde::Serialize;

use accessibility::{Accessibility, Element};

use crate::application::Application;
use crate::ui::instance::select_instance;
use crate::ui::output::{prepare_output, write_new};
use crate::ui::permission::Permission;
use crate::ui::request::{Selection, UiRequest};
use crate::ui::response::{
    CapturedImage, CapturedWindow, ObservedApplication, ScreenshotResponse, TreeResponse,
    WindowsResponse,
};
use crate::ui::tree::read_tree;
use crate::ui::window::{Window, select_window};

/// Runs one `recon ui` command and returns its result as JSON.
///
/// Recon becomes responsible for its own privacy requests first, then resolves the application,
/// the instance, the output file, and the window, so that a mistaken argument is reported before
/// macOS is ever asked for a permission.
pub fn observe(request: &UiRequest) -> Result<String, String> {
    responsibility::take_responsibility()?;
    match request {
        UiRequest::Windows { selection } => windows(selection),
        UiRequest::Tree { selection, window } => tree(selection, *window),
        UiRequest::Screenshot { selection, window, out } => screenshot(selection, *window, out),
    }
}

fn windows(selection: &Selection) -> Result<String, String> {
    with_instance(selection, |application, pid| {
        permissions::require(&[Permission::ScreenRecording])?;
        let windows = window_server::windows_owned_by(pid)?;
        encode(&WindowsResponse { application: observed(application, pid), windows })
    })
}

fn tree(selection: &Selection, window: Option<u32>) -> Result<String, String> {
    with_instance(selection, |application, pid| {
        if let Some(id) = window {
            select_window(&window_server::windows_owned_by(pid)?, id, pid)?;
        }
        permissions::require(&[Permission::Accessibility])?;

        accessibility::bound_messaging();
        let application_element = accessibility::application_element(pid)?;
        let root = match window {
            Some(id) => window_element::window_element(&application_element, pid, id)?,
            None => application_element,
        };

        let tree = read_tree(&mut Accessibility::new(), Element::new(root));
        encode(&TreeResponse {
            application: observed(application, pid),
            window_id: window,
            truncated: tree.truncated,
            root: tree.root,
        })
    })
}

fn screenshot(selection: &Selection, window: Option<u32>, out: &Path) -> Result<String, String> {
    with_instance(selection, |application, pid| {
        let out = prepare_output(out, &current_directory()?)?;
        let window = match window {
            Some(id) => {
                let windows = window_server::windows_owned_by(pid)?;
                let window = select_window(&windows, id, pid)?.clone();
                permissions::require(&[Permission::ScreenRecording])?;
                window
            }
            None => {
                permissions::require(&[Permission::ScreenRecording, Permission::Accessibility])?;
                main_window(application, pid)?
            }
        };

        let captured = capture::capture(window.id)?;
        write_new(&out, &captured.png)?;
        encode(&ScreenshotResponse::new(
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
        ))
    })
}

/// The window the application itself reports as its main one, as the window server lists it.
fn main_window(application: &Application, pid: u32) -> Result<Window, String> {
    accessibility::bound_messaging();
    let application_element = accessibility::application_element(pid)?;
    let id = window_element::main_window_id(&application_element, pid)?.ok_or_else(|| {
        format!(
            "{} reports no main window; choose a window from recon ui windows and pass --window",
            application.path.display()
        )
    })?;
    let windows = window_server::windows_owned_by(pid)?;
    Ok(select_window(&windows, id, pid)?.clone())
}

fn with_instance<T>(
    selection: &Selection,
    observe_instance: impl FnOnce(&Application, u32) -> Result<T, String>,
) -> Result<T, String> {
    super::with_target(&selection.target, |application| {
        let pid = select_instance(&application.path, &application.running_pids, selection.pid)?;
        observe_instance(application, pid)
    })
}

fn observed(application: &Application, pid: u32) -> ObservedApplication<'_> {
    ObservedApplication { path: &application.path, pid }
}

fn current_directory() -> Result<PathBuf, String> {
    env::current_dir().map_err(|error| format!("cannot read the current directory: {error}"))
}

fn encode(response: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(response).map_err(|error| format!("cannot encode response: {error}"))
}
