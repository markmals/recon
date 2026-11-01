//! The Recon-owned native loader.
//!
//! Before any effect on a target, the loader binds to one kernel-verified process incarnation and
//! confirms the target shares this process's dyld shared cache, so later code execution can resolve
//! libSystem entry points at identical addresses. This module owns that pre-effect binding; memory
//! writes and thread creation are built on top of it.

mod incarnation;
mod mach;
mod shared_cache;

use std::fmt;

pub use incarnation::{Incarnation, TaskHandle, bind, rebind};
pub use shared_cache::{SharedCache, verify as verify_shared_cache};

/// Why the loader refused to bind to or inspect a target.
///
/// These are internal to the native loader. The CLI maps them onto the published `INSPECTION_*`
/// error contract when sessions are wired up; keeping them separate here lets the loader be tested
/// without the command surface.
#[derive(Debug)]
pub enum LoaderError {
    /// The task control port could not be acquired. Carries the Mach `kern_return_t`.
    TaskUnavailable(i32),
    /// The task's audit token could not be read. Carries the Mach `kern_return_t`.
    AuditTokenUnavailable(i32),
    /// The running process is not the incarnation the caller selected: the pid was reused, or the
    /// port now names a different task.
    IncarnationMismatch { expected: Incarnation, found: Incarnation },
    /// The target does not share this process's dyld shared cache, so libSystem symbols cannot be
    /// resolved at identical addresses. Injection is unavailable for it.
    SharedCacheMismatch,
    /// dyld process information for the target could not be read. Carries the Mach `kern_return_t`.
    DyldInfoUnavailable(i32),
}

impl fmt::Display for LoaderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskUnavailable(kr) => {
                write!(formatter, "could not acquire the task control port (kern_return {kr})")
            }
            Self::AuditTokenUnavailable(kr) => {
                write!(formatter, "could not read the task's audit token (kern_return {kr})")
            }
            Self::IncarnationMismatch { expected, found } => {
                write!(formatter, "process incarnation changed: expected {expected}, found {found}")
            }
            Self::SharedCacheMismatch => {
                write!(formatter, "the target does not share this process's dyld shared cache")
            }
            Self::DyldInfoUnavailable(kr) => {
                write!(formatter, "could not read the target's dyld information (kern_return {kr})")
            }
        }
    }
}

impl std::error::Error for LoaderError {}

/// A target bound for inspection: a verified incarnation and the shared cache both processes map.
#[derive(Debug)]
pub struct Target {
    pub handle: TaskHandle,
    pub shared_cache: SharedCache,
}

/// Bind to the process now running as `pid` and confirm it can be inspected: acquire its control
/// port, record its incarnation, and verify the shared cache. Reads only; nothing is written.
pub fn bind_target(pid: libc::pid_t) -> Result<Target, LoaderError> {
    let handle = bind(pid)?;
    let shared_cache = verify_shared_cache(handle.task())?;
    Ok(Target { handle, shared_cache })
}

/// Bind to `pid` and require it to still be `expected` before confirming the shared cache. Used
/// after a session has recorded an incarnation, so a reused pid is refused.
pub fn rebind_target(pid: libc::pid_t, expected: Incarnation) -> Result<Target, LoaderError> {
    let handle = rebind(pid, expected)?;
    let shared_cache = verify_shared_cache(handle.task())?;
    Ok(Target { handle, shared_cache })
}
