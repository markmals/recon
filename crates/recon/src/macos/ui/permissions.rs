//! Screen Recording and Accessibility, as macOS grants them to the `recon` executable itself.

use std::env;
use std::ffi::{c_int, c_void};
use std::path::PathBuf;

use libc::pid_t;
use objc2_application_services::{
    AXIsProcessTrusted, AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFType};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

use crate::ui::permission::{Permission, Signature, missing_permissions_message};

/// Reports the code-signing status of a process; see `cs_blobs.h` in the kernel sources.
const CS_OPS_STATUS: u32 = 0;
const CS_ADHOC: u32 = 0x0000_0002;

unsafe extern "C" {
    fn csops(pid: pid_t, ops: u32, useraddr: *mut c_void, usersize: usize) -> c_int;
}

/// Checks every permission the command needs, asking macOS to request the missing ones.
///
/// Checking never prompts; requesting may, and macOS stays silent when the user denied the
/// permission earlier. Either way a missing permission is reported rather than worked around.
pub fn require(needed: &[Permission]) -> Result<(), String> {
    let missing: Vec<Permission> =
        needed.iter().copied().filter(|permission| !granted(*permission)).collect();
    if missing.is_empty() {
        return Ok(());
    }

    for permission in &missing {
        request(*permission);
    }
    Err(missing_permissions_message(&missing, &executable()?, signature()))
}

fn granted(permission: Permission) -> bool {
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

fn executable() -> Result<PathBuf, String> {
    let executable = env::current_exe()
        .map_err(|error| format!("cannot find the running recon executable: {error}"))?;
    Ok(executable.canonicalize().unwrap_or(executable))
}

/// An ad-hoc signature's designated requirement is the code hash itself, so macOS forgets a grant
/// as soon as the executable is rebuilt. The error says so rather than leaving it a mystery.
fn signature() -> Signature {
    let mut flags: u32 = 0;
    let size = size_of_val(&flags);
    // SAFETY: `csops` writes `size` bytes of status flags for the given process.
    let status = unsafe { csops(libc::getpid(), CS_OPS_STATUS, (&raw mut flags).cast(), size) };
    if status == 0 && flags & CS_ADHOC != 0 { Signature::AdHoc } else { Signature::Stable }
}
