use std::path::Path;

use serde::Serialize;

use crate::ui::tree::Node;
use crate::ui::window::{Bounds, Window};

const CAPTURE_METHOD: &str = "skylight-hardware-capture";

#[derive(Debug, Serialize)]
pub struct ObservedApplication<'a> {
    pub path: &'a Path,
    pub pid: u32,
}

#[derive(Debug, Serialize)]
pub struct WindowsResponse<'a> {
    pub application: ObservedApplication<'a>,
    pub windows: Vec<Window>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeResponse<'a> {
    pub application: ObservedApplication<'a>,
    pub window_id: Option<u32>,
    pub truncated: bool,
    pub root: Node,
}

/// The window as it was at the moment of capture.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapturedWindow<'a> {
    pub id: u32,
    pub title: Option<&'a str>,
    pub bounds: Bounds,
    pub on_screen: bool,
}

#[derive(Debug, Serialize)]
pub struct CapturedImage<'a> {
    pub path: &'a Path,
    pub width: u32,
    pub height: u32,
    pub uniform: bool,
}

#[derive(Debug, Serialize)]
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
