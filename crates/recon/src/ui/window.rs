//! Windows as the window server lists them, and how a command picks one.

use std::fmt;

use schemars::JsonSchema;
use serde::Serialize;

use crate::application::Pid;
use crate::error::{ErrorCode, Failure, NotFound};

/// A rectangle in global screen points, with the origin at the top left of the main display.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A window server window number: the `CGWindowID` that `--window` names and results report.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct WindowId(pub u32);

impl fmt::Display for WindowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A window-server window, in the order and the global points the window server reports.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Window {
    pub id: WindowId,
    /// The owning process is how a window is selected, not something the result reports.
    #[serde(skip)]
    pub owner_pid: Pid,
    pub title: Option<String>,
    pub layer: i32,
    pub bounds: Bounds,
    pub on_screen: bool,
    pub alpha: f64,
}

/// An empty title is no title, so a window titled `""` reports `null`.
pub fn title_from(raw: Option<String>) -> Option<String> {
    raw.filter(|title| !title.is_empty())
}

/// The windows `pid` owns, in the order of `all`.
pub fn windows_of(all: Vec<Window>, pid: Pid) -> Vec<Window> {
    all.into_iter().filter(|window| window.owner_pid == pid).collect()
}

/// The window `id`, which must belong to `pid`.
///
/// # Errors
///
/// [`ErrorCode::NotFound`] when `windows` holds no window `id` owned by `pid`.
pub fn select_window(windows: Vec<Window>, id: WindowId, pid: Pid) -> Result<Window, Failure> {
    windows.into_iter().find(|window| window.id == id && window.owner_pid == pid).ok_or_else(|| {
        Failure::new(
            ErrorCode::NotFound,
            format!("window {id} is not a window of process {pid}"),
            NotFound {
                kind: "window",
                pid: Some(pid.0),
                window_id: Some(id.0),
                ..Default::default()
            }
            .details(),
        )
    })
}
