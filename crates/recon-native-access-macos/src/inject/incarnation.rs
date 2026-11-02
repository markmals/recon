//! Naming one kernel-verified process incarnation.
//!
//! A pid alone is not an identity: the kernel reuses pids, and exec gives the same pid a new
//! generation. The audit token's pid generation tells one incarnation from the next, so the loader
//! can refuse to act on any process but the one Recon selected.

use std::fmt;

use libc::{mach_port_t, pid_t};

use super::IncarnationError;
use super::mach::{self, AuditToken, KERN_SUCCESS, TASK_AUDIT_TOKEN, TASK_AUDIT_TOKEN_COUNT};

/// A kernel-verified identity for one running process: its pid and the pid generation that
/// distinguishes it from a later process reusing the same number.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Incarnation {
    pub pid: pid_t,
    pub pidversion: u32,
}

impl fmt::Debug for Incarnation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "pid {} generation {}", self.pid, self.pidversion)
    }
}

impl fmt::Display for Incarnation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, formatter)
    }
}

/// Identify the incarnation now running as `pid` from its task name port. The name port grants
/// no control over the task, so this needs no debugger rights and never touches the process's
/// memory or threads.
pub fn identify(pid: pid_t) -> Result<Incarnation, IncarnationError> {
    let name = task_name_for_pid(pid)?;
    let incarnation = read_incarnation(name);
    // SAFETY: `name` is a send right acquired above and released exactly once, here.
    unsafe {
        mach::mach_port_deallocate(mach::mach_task_self(), name);
    }
    let incarnation = incarnation?;
    if incarnation.pid == pid {
        Ok(incarnation)
    } else {
        Err(IncarnationError::PidMismatch { requested: pid, found: incarnation })
    }
}

fn task_name_for_pid(pid: pid_t) -> Result<mach_port_t, IncarnationError> {
    let mut name: mach_port_t = 0;
    // SAFETY: `name` is a valid out pointer; the trap writes the name port on success.
    let kr = unsafe { mach::task_name_for_pid(mach::mach_task_self(), pid, &mut name) };
    if kr == KERN_SUCCESS { Ok(name) } else { Err(IncarnationError::NameUnavailable(kr)) }
}

fn read_incarnation(task: mach_port_t) -> Result<Incarnation, IncarnationError> {
    let mut token = AuditToken { val: [0; 8] };
    let mut count = TASK_AUDIT_TOKEN_COUNT;
    // SAFETY: `token` holds eight words and `count` is initialized to that length, as task_info
    // requires for the audit-token flavor.
    let kr = unsafe {
        mach::task_info(task, TASK_AUDIT_TOKEN, token.val.as_mut_ptr().cast(), &mut count)
    };
    if kr != KERN_SUCCESS {
        return Err(IncarnationError::AuditTokenUnavailable(kr));
    }
    Ok(Incarnation { pid: token.val[5] as pid_t, pidversion: token.val[7] })
}
