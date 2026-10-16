use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A window-server window, in the order and the global points the window server reports.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub id: u32,
    /// The owning process is how a window is selected, not something the result reports.
    #[serde(skip)]
    pub owner_pid: u32,
    pub title: Option<String>,
    pub layer: i32,
    pub bounds: Bounds,
    pub on_screen: bool,
    pub alpha: f64,
}

pub fn title_from(raw: Option<String>) -> Option<String> {
    raw.filter(|title| !title.is_empty())
}

pub fn windows_of(all: Vec<Window>, pid: u32) -> Vec<Window> {
    all.into_iter().filter(|window| window.owner_pid == pid).collect()
}

pub fn select_window(windows: &[Window], id: u32, pid: u32) -> Result<&Window, String> {
    windows
        .iter()
        .find(|window| window.id == id && window.owner_pid == pid)
        .ok_or_else(|| format!("window {id} is not a window of process {pid}"))
}
