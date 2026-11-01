//! The window server's own account of a Mac: which windows exist, who owns them, where they
//! are, and what one of them currently shows.
//!
//! Every observation here comes from WindowServer and SkyLight, including the private calls that
//! capture a window without bringing it forward. Nothing here walks an accessibility tree or an
//! NSView hierarchy: those are other evidence, from other sources, in other crates.

pub mod image;
pub mod window;

#[cfg(target_os = "macos")]
mod private;

#[cfg(target_os = "macos")]
pub mod capture;
#[cfg(target_os = "macos")]
pub mod inventory;

/// The private window server interfaces this crate depends on, and whether this macOS still has
/// each one. Looking a symbol up neither calls it nor needs its signature.
#[cfg(target_os = "macos")]
#[must_use]
pub fn interfaces() -> Vec<(&'static str, &'static str, bool)> {
    [
        ("window-server-connection", c"CGSMainConnectionID"),
        ("skylight-capture", c"SLSHWCaptureWindowList"),
    ]
    .into_iter()
    .map(|(interface, symbol)| {
        (interface, symbol.to_str().expect("an ASCII symbol name"), private::available(symbol))
    })
    .collect()
}
