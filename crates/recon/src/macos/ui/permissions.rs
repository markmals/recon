//! Screen Recording and Accessibility, as macOS grants them to the `recon` executable itself.

use objc2_application_services::{
    AXIsProcessTrusted, AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFType};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

use super::signing::facts;
use crate::context::executable_path;
use crate::error::Failure;
use crate::ui::permission::{Permission, permission_denied};

/// Checks every permission the command needs, asking macOS to request the missing ones.
///
/// Checking never prompts; requesting may, and macOS stays silent when the user denied the
/// permission earlier. Either way a missing permission is reported rather than worked around.
pub(super) fn require(needed: &[Permission]) -> Result<(), Failure> {
    let missing: Vec<Permission> =
        needed.iter().copied().filter(|permission| !granted(*permission)).collect();
    if missing.is_empty() {
        return Ok(());
    }

    for permission in &missing {
        request(*permission);
    }
    Err(permission_denied(&missing, &executable_path()?, facts()?.ad_hoc))
}

/// Whether macOS has granted the permission to this process, asked in the way that never
/// prompts: this is the only question `recon context` asks of either permission.
pub(super) fn granted(permission: Permission) -> bool {
    match permission {
        Permission::ScreenRecording => CGPreflightScreenCaptureAccess(),
        // SAFETY: reads this process's accessibility trust; it neither prompts nor changes state.
        Permission::Accessibility => unsafe { AXIsProcessTrusted() },
    }
}

fn request(permission: Permission) {
    match permission {
        Permission::ScreenRecording => {
            CGRequestScreenCaptureAccess();
        }
        Permission::Accessibility => {
            // SAFETY: the static key is a CFString, and the options hold Core Foundation values.
            let prompt: &CFType = unsafe { kAXTrustedCheckOptionPrompt };
            let options = CFDictionary::from_slices(&[prompt], &[CFBoolean::new(true) as &CFType]);
            // SAFETY: the dictionary holds the documented option key and a CFBoolean.
            unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) };
        }
    }
}
