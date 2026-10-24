use std::fmt;

use serde::Serialize;

use crate::application::Pid;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A window server window number: the `CGWindowID` that `--window` names and results report.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WindowId(pub u32);

impl fmt::Display for WindowId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A window-server window, in the order and the global points the window server reports.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
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

pub fn title_from(raw: Option<String>) -> Option<String> {
    raw.filter(|title| !title.is_empty())
}

pub fn windows_of(all: Vec<Window>, pid: Pid) -> Vec<Window> {
    all.into_iter().filter(|window| window.owner_pid == pid).collect()
}

pub fn select_window(windows: Vec<Window>, id: WindowId, pid: Pid) -> Result<Window, String> {
    windows
        .into_iter()
        .find(|window| window.id == id && window.owner_pid == pid)
        .ok_or_else(|| format!("window {id} is not a window of process {pid}"))
}
