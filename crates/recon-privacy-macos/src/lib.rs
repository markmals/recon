//! Recon's own privacy identity on a Mac: which executable macOS records a grant against, how
//! that executable is signed, whether macOS holds Recon responsible for its own requests, and
//! which permissions it currently has.
//!
//! The boundary this crate keeps truthful is the one between Recon and whatever launched it.
//! Re-executing with the disclaim attribute makes the grants belong to the `recon` executable
//! under every launcher, and the child's output and signals are forwarded so that the
//! re-execution stays invisible to the person who ran the command.

pub mod identity;
pub mod permission;

#[cfg(target_os = "macos")]
mod child_output;
#[cfg(target_os = "macos")]
mod forwarding;
#[cfg(target_os = "macos")]
mod private;
#[cfg(target_os = "macos")]
mod signing;

#[cfg(target_os = "macos")]
pub mod permissions;
#[cfg(target_os = "macos")]
pub mod responsibility;

/// The private interfaces this crate depends on, and whether this macOS still has each one.
/// Looking a symbol up neither calls it nor needs its signature.
#[cfg(target_os = "macos")]
#[must_use]
pub fn interfaces() -> Vec<(&'static str, &'static str, bool)> {
    [
        ("responsibility-query", c"responsibility_get_pid_responsible_for_pid"),
        ("responsibility-disclaim", c"responsibility_spawnattrs_setdisclaim"),
        ("code-signing-query", c"csops"),
    ]
    .into_iter()
    .map(|(interface, symbol)| {
        (interface, symbol.to_str().expect("an ASCII symbol name"), private::available(symbol))
    })
    .collect()
}
