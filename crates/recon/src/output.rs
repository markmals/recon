//! Final-response delivery without a buffered stdout retry during process shutdown.

use std::fs::File;
use std::io;

use recon_core::error::Failure;
use serde::Serialize;
#[cfg(not(windows))]
use std::os::fd::AsFd;
#[cfg(windows)]
use std::os::windows::io::AsHandle;

/// An independently owned handle that writes directly to the inherited stdout sink.
///
/// Rust's global stdout can retain a failed line-buffer flush and retry it at shutdown. Cloning
/// its handle avoids that buffer without taking ownership of, or closing, the inherited handle.
pub fn stdout() -> io::Result<File> {
    let stdout = io::stdout();
    #[cfg(not(windows))]
    let handle = stdout.as_fd().try_clone_to_owned()?;
    #[cfg(windows)]
    let handle = stdout.as_handle().try_clone_to_owned()?;
    Ok(File::from(handle))
}

/// One response as the compact JSON object a command prints.
///
/// # Errors
///
/// [`recon_core::error::ErrorCode::Internal`] when the response cannot be encoded.
pub fn encode(response: &impl Serialize) -> Result<String, Failure> {
    serde_json::to_string(response)
        .map_err(|error| Failure::internal(format!("cannot encode response: {error}")))
}
