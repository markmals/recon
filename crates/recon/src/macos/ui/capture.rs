//! Capturing the current pixels of exactly one window, through the private SkyLight hardware
//! capture that reaches occluded, off-Space, minimized, and hidden windows.

use std::ffi::c_int;
use std::ptr::NonNull;

use objc2_core_foundation::{CFArray, CFMutableData, CFRetained, CFString};
use objc2_core_graphics::{CGDataProvider, CGImage, CGWindowID};
use objc2_image_io::CGImageDestination;
use serde_json::json;

use super::private;
use crate::error::{ErrorCode, Failure};
use crate::ui::image::{PixelLayout, is_uniform};
use crate::ui::window::WindowId;

type MainConnection = unsafe extern "C" fn() -> c_int;
type CaptureWindowList = unsafe extern "C" fn(c_int, *const CGWindowID, u32, u32) -> *mut CFArray;

const PNG: &str = "public.png";

/// One window's pixels, ready to be written as a PNG file.
pub(super) struct Capture {
    pub width: u32,
    pub height: u32,
    pub uniform: bool,
    pub png: Vec<u8>,
}

/// Captures the window server's current content for one window.
pub(super) fn capture(window_id: WindowId) -> Result<Capture, Failure> {
    let image = window_image(window_id)?;
    let width = CGImage::width(Some(&image));
    let height = CGImage::height(Some(&image));

    Ok(Capture {
        width: u32::try_from(width).map_err(|_| oversized(window_id))?,
        height: u32::try_from(height).map_err(|_| oversized(window_id))?,
        uniform: uniform(&image, width, height, window_id)?,
        png: png(&image, window_id)?,
    })
}

fn window_image(window_id: WindowId) -> Result<CFRetained<CGImage>, Failure> {
    // SAFETY: `MainConnection` is the signature of `CGSMainConnectionID`.
    let connection: MainConnection = unsafe { private::function(c"CGSMainConnectionID") }
        .ok_or_else(|| unavailable("window-server-connection", "CGSMainConnectionID"))?;
    // SAFETY: `CaptureWindowList` is the signature of `SLSHWCaptureWindowList`.
    let capture_windows: CaptureWindowList =
        unsafe { private::function(c"SLSHWCaptureWindowList") }
            .ok_or_else(|| unavailable("skylight-capture", "SLSHWCaptureWindowList"))?;

    let windows = [window_id.0];
    // SAFETY: the main connection takes no arguments, and the capture reads the one window the
    // list names and returns an owned array.
    let images = unsafe { capture_windows(connection(), windows.as_ptr(), 1, 0) };
    let images = NonNull::new(images).ok_or_else(|| no_image(window_id))?;
    // SAFETY: the array is owned, and hardware capture fills it with images.
    let images: CFRetained<CFArray<CGImage>> =
        unsafe { CFRetained::cast_unchecked(CFRetained::from_raw(images)) };
    images.get(0).ok_or_else(|| no_image(window_id))
}

/// A capture of an invisible or fully transparent window is uniform; Recon records that rather
/// than treating it as a failure.
fn uniform(
    image: &CGImage,
    width: usize,
    height: usize,
    window_id: WindowId,
) -> Result<bool, Failure> {
    let bits_per_pixel = CGImage::bits_per_pixel(Some(image));
    if bits_per_pixel == 0 || !bits_per_pixel.is_multiple_of(8) {
        return Err(capture_failed(
            window_id,
            Stage::Pixels,
            format!("macOS returned window {window_id} in an unsupported pixel format"),
        ));
    }
    let provider = CGImage::data_provider(Some(image));
    let pixels = CGDataProvider::data(provider.as_deref()).ok_or_else(|| {
        capture_failed(
            window_id,
            Stage::Pixels,
            format!("macOS returned no pixels for window {window_id}"),
        )
    })?;
    // SAFETY: the copied pixels are not mutated while they are read.
    let pixels = unsafe { pixels.as_bytes_unchecked() };
    let layout = PixelLayout {
        width,
        height,
        bytes_per_row: CGImage::bytes_per_row(Some(image)),
        bytes_per_pixel: bits_per_pixel / 8,
    };
    Ok(is_uniform(pixels, layout))
}

fn png(image: &CGImage, window_id: WindowId) -> Result<Vec<u8>, Failure> {
    let encoded = CFMutableData::new(None, 0).ok_or_else(|| encoding_failed(window_id))?;
    // SAFETY: a PNG destination holding one image, with no options.
    let destination = unsafe {
        CGImageDestination::with_data(&encoded, &CFString::from_static_str(PNG), 1, None)
    }
    .ok_or_else(|| encoding_failed(window_id))?;
    // SAFETY: the image outlives the destination, and no per-image options are given.
    unsafe { destination.add_image(image, None) };
    // SAFETY: finalizing writes the encoded image into `encoded`.
    if !unsafe { destination.finalize() } {
        return Err(encoding_failed(window_id));
    }
    Ok(encoded.to_vec())
}

/// How far a capture got before it failed, which is what an agent needs to tell a window that
/// cannot be captured from a capture that cannot be encoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    Capture,
    Pixels,
    Encode,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Stage::Capture => "capture",
            Stage::Pixels => "pixels",
            Stage::Encode => "encode",
        }
    }
}

/// A private interface the capture needs that this macOS version does not have.
fn unavailable(interface: &str, symbol: &str) -> Failure {
    Failure::new(
        ErrorCode::InterfaceUnavailable,
        format!("window capture is unavailable on this macOS version ({symbol} not found)"),
        json!({ "interface": interface, "symbol": symbol }),
    )
}

fn capture_failed(window_id: WindowId, stage: Stage, message: String) -> Failure {
    Failure::new(
        ErrorCode::CaptureFailed,
        message,
        json!({ "windowId": window_id.0, "stage": stage.name(), "detail": null }),
    )
}

fn no_image(window_id: WindowId) -> Failure {
    capture_failed(
        window_id,
        Stage::Capture,
        format!("macOS returned no image for window {window_id}"),
    )
}

fn oversized(window_id: WindowId) -> Failure {
    capture_failed(
        window_id,
        Stage::Capture,
        format!("macOS returned an image of window {window_id} that is too large to report"),
    )
}

fn encoding_failed(window_id: WindowId) -> Failure {
    capture_failed(
        window_id,
        Stage::Encode,
        format!("macOS could not encode the capture of window {window_id} as PNG"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_capture_failure_says_how_far_the_capture_got() {
        for (failure, stage) in [
            (no_image(WindowId(53510)), "capture"),
            (oversized(WindowId(53510)), "capture"),
            (encoding_failed(WindowId(53510)), "encode"),
        ] {
            assert_eq!(failure.code, ErrorCode::CaptureFailed);
            assert!(failure.retryable, "a later capture of the same window can succeed");
            assert_eq!(
                failure.details.expect("the data needed to recover"),
                json!({ "windowId": 53510, "stage": stage, "detail": null })
            );
        }
    }

    #[test]
    fn an_absent_capture_interface_names_the_symbol_recon_looked_for() {
        let failure = unavailable("skylight-capture", "SLSHWCaptureWindowList");

        assert_eq!(failure.code, ErrorCode::InterfaceUnavailable);
        assert!(!failure.retryable, "a missing interface stays missing");
        assert_eq!(
            failure.details.expect("the data needed to recover"),
            json!({ "interface": "skylight-capture", "symbol": "SLSHWCaptureWindowList" })
        );
    }
}
