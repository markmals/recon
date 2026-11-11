//! What macOS Accessibility says one application's interface is: its elements, their attributes,
//! the element behind one window server window, and the bounded tree a read produces.
//!
//! This is the accessibility source's own evidence. An element is whatever the application
//! chooses to publish through AX, which is neither the window server's inventory nor the
//! application's own view hierarchy, and nothing here claims an element and a native object are
//! the same object.

pub mod tree;

#[cfg(target_os = "macos")]
pub mod interaction;

#[cfg(target_os = "macos")]
mod attributes;
#[cfg(target_os = "macos")]
mod private;

#[cfg(target_os = "macos")]
pub mod element;
#[cfg(target_os = "macos")]
pub mod window_element;

#[cfg(all(test, target_os = "macos"))]
mod menu_bar_tests;

/// The private accessibility interfaces this crate depends on, and whether this macOS still has
/// each one. Looking a symbol up neither calls it nor needs its signature.
#[cfg(target_os = "macos")]
#[must_use]
pub fn interfaces() -> Vec<(&'static str, &'static str, bool)> {
    [
        ("accessibility-window-id", c"_AXUIElementGetWindow"),
        ("accessibility-remote-token", c"_AXUIElementCreateWithRemoteToken"),
    ]
    .into_iter()
    .map(|(interface, symbol)| {
        (interface, symbol.to_str().expect("an ASCII symbol name"), private::available(symbol))
    })
    .collect()
}
