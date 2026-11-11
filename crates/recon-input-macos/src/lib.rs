//! Window-targeted macOS input and the portable grammar that describes it.

pub mod chord;
pub mod request;

#[cfg(target_os = "macos")]
mod event;
#[cfg(target_os = "macos")]
mod failure;
#[cfg(target_os = "macos")]
mod host;
#[cfg(target_os = "macos")]
mod key_window;
#[cfg(target_os = "macos")]
mod keyboard;
#[cfg(target_os = "macos")]
mod layout;
#[cfg(target_os = "macos")]
mod mouse;
#[cfg(target_os = "macos")]
mod private;
#[cfg(target_os = "macos")]
mod scroll;
#[cfg(target_os = "macos")]
mod target;
#[cfg(target_os = "macos")]
mod toolkit;

#[cfg(target_os = "macos")]
pub use host::{require_secure_input_disabled, require_unlocked};
#[cfg(target_os = "macos")]
pub use key_window::{frontmost_pid, make_appkit_key};
#[cfg(target_os = "macos")]
pub use keyboard::send as send_chord;
#[cfg(target_os = "macos")]
pub use layout::keycode;
#[cfg(target_os = "macos")]
pub use mouse::click;
#[cfg(target_os = "macos")]
pub use scroll::scroll;
#[cfg(target_os = "macos")]
pub use target::Target;
#[cfg(target_os = "macos")]
pub use toolkit::{Toolkit, read as read_toolkit};

#[cfg(target_os = "macos")]
#[must_use]
pub fn interfaces() -> Vec<(&'static str, &'static str, bool)> {
    [
        ("window-event-fields", c"SLEventSetIntegerValueField"),
        ("window-event-location", c"CGEventSetWindowLocation"),
        ("target-key-window-records", c"SLPSPostEventRecordTo"),
        ("window-owner", c"SLSGetWindowOwner"),
        ("window-owner-psn", c"SLSGetConnectionPSN"),
    ]
    .into_iter()
    .map(|(interface, symbol)| {
        (interface, symbol.to_str().expect("an ASCII symbol name"), private::available(symbol))
    })
    .collect()
}
