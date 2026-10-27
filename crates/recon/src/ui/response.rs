//! The JSON object each `recon ui` command prints on success.

use std::path::Path;

use schemars::JsonSchema;
use serde::Serialize;

use crate::application::Pid;
use crate::ui::tree::Node;
use crate::ui::window::{Bounds, Window, WindowId};

const CAPTURE_METHOD: &str = "skylight-hardware-capture";

/// The bundle and the running instance a result describes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ObservedApplication<'a> {
    pub path: &'a Path,
    pub pid: Pid,
}

/// What `recon ui windows` prints.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WindowsResponse<'a> {
    pub application: ObservedApplication<'a>,
    pub windows: Vec<Window>,
}

/// What `recon ui tree` prints. `window_id` is the `--window` the command was given, if any.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TreeResponse<'a> {
    pub application: ObservedApplication<'a>,
    pub window_id: Option<WindowId>,
    pub truncated: bool,
    pub root: Node,
}

/// The window as it was at the moment of capture.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapturedWindow<'a> {
    pub id: WindowId,
    pub title: Option<&'a str>,
    pub bounds: Bounds,
    pub on_screen: bool,
}

/// The PNG file a capture wrote, in backing pixels. It is `uniform` when every pixel is the same,
/// as in a capture of an invisible or fully transparent window.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CapturedImage<'a> {
    pub path: &'a Path,
    pub width: u32,
    pub height: u32,
    pub uniform: bool,
}

/// What `recon ui screenshot` prints.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotResponse<'a> {
    pub application: ObservedApplication<'a>,
    pub window: CapturedWindow<'a>,
    pub image: CapturedImage<'a>,
    /// Every capture result names the mechanism that produced it; there is only one.
    method: &'static str,
}

impl<'a> ScreenshotResponse<'a> {
    pub fn new(
        application: ObservedApplication<'a>,
        window: CapturedWindow<'a>,
        image: CapturedImage<'a>,
    ) -> Self {
        Self { application, window, image, method: CAPTURE_METHOD }
    }
}
