//! The applications installed and running on this Mac: what a bundle says about itself, which
//! copies exist, which of them are running, which one a selector names, and how one is launched.
//!
//! An application here is a bundle at one path with the instances observed for exactly that
//! path. Copies at other paths are other applications, and a selector that names more than one
//! is ambiguous rather than resolved by preference.

pub mod application;
pub mod response;
pub mod selection;

#[cfg(target_os = "macos")]
mod metadata;

#[cfg(target_os = "macos")]
pub mod discovery;
#[cfg(target_os = "macos")]
pub mod workspace;
