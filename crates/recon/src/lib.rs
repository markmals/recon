//! The internals of the `recon` executable.
//!
//! The library exists so that integration tests can reach what the executable assembles; it is
//! not a supported API and is never published. [`request`] parses the command line, [`cli`] owns
//! the one command registry that help and schemas come from, [`ui`] holds the `recon ui` grammar
//! and the JSON each command prints, [`context`] composes what this host says about Recon, and
//! [`correlate`] relates two sources' evidence without making either own the other.
//!
//! The capabilities themselves live in their own crates. On a Mac the `investigate` and `observe`
//! modules assemble them into the commands a person runs; neither exists on other hosts.

pub mod cli;
pub mod context;
pub mod correlate;
pub mod output;
pub mod request;
pub mod ui;

#[cfg(target_os = "macos")]
pub mod investigate;
#[cfg(target_os = "macos")]
pub mod observe;
