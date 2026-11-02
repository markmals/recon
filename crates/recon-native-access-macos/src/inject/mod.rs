//! The process incarnation Recon hands its native loader.
//!
//! The loader (`loader/recon_loader.c`) is the only Recon process that controls a target: it
//! acquires the task control port, checks this incarnation again against that port's audit token,
//! and confirms the shared cache before any effect. Recon itself only names the incarnation. The
//! task name port reveals that identity without granting control, so Recon needs no debugger
//! entitlement of its own.

mod incarnation;
mod mach;

use std::fmt;

pub use incarnation::{Incarnation, identify};

/// Why the incarnation running as a pid could not be identified.
#[derive(Debug)]
pub enum IncarnationError {
    /// The task name port could not be acquired, as for a process that has exited. Carries the
    /// Mach `kern_return_t`.
    NameUnavailable(i32),
    /// The task's audit token could not be read. Carries the Mach `kern_return_t`.
    AuditTokenUnavailable(i32),
    /// The name port acquired for one pid names a different process.
    PidMismatch { requested: libc::pid_t, found: Incarnation },
}

impl fmt::Display for IncarnationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NameUnavailable(kr) => {
                write!(formatter, "could not acquire the task name port (kern_return {kr})")
            }
            Self::AuditTokenUnavailable(kr) => {
                write!(formatter, "could not read the task's audit token (kern_return {kr})")
            }
            Self::PidMismatch { requested, found } => {
                write!(formatter, "the task name port for pid {requested} names {found}")
            }
        }
    }
}

impl std::error::Error for IncarnationError {}
