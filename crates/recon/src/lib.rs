//! The internals of the `recon` executable.
//!
//! The library exists so that integration tests can reach Recon's platform-independent logic; it
//! is not a supported API and is never published. [`request`] parses the command line,
//! [`application`] and [`ui`] model what Recon observes independently of the platform, and
//! `macos` observes it on a Mac.

pub mod application;
pub mod cli;
pub mod context;
pub mod error;
pub mod request;
pub mod ui;

#[cfg(target_os = "macos")]
pub mod macos;
