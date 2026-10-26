//! Final-response delivery without a buffered stdout retry during process shutdown.

use std::fs::File;
use std::io;
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
