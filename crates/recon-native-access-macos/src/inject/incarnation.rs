//! Binding to one kernel-verified process incarnation.
//!
//! A pid alone is not an identity: the kernel reuses pids, so a port acquired for one process can
//! outlive it and a later process can take its number. The loader binds to a task control port and
//! records the audit token's pid generation, so every later effect can be refused unless it still
//! names the same incarnation.

use std::fmt;

use libc::{mach_port_t, pid_t};

use super::LoaderError;
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

/// A task control port bound to a specific process incarnation. Dropping it releases the port.
#[derive(Debug)]
pub struct TaskHandle {
    task: mach_port_t,
    incarnation: Incarnation,
}

impl TaskHandle {
    /// The control port, for the memory and thread operations built on top of this binding.
    pub fn task(&self) -> mach_port_t {
        self.task
    }

    /// The incarnation this handle is bound to.
    pub fn incarnation(&self) -> Incarnation {
        self.incarnation
    }

    /// Re-read the task's audit token and confirm it still names the bound incarnation. A task that
    /// has exited makes the port unusable; a reused pid changes the generation. Either way the
    /// caller must refuse to act on a replacement process.
    pub fn revalidate(&self) -> Result<(), LoaderError> {
        let found = read_incarnation(self.task)?;
        if found == self.incarnation {
            Ok(())
        } else {
            Err(LoaderError::IncarnationMismatch { expected: self.incarnation, found })
        }
    }
}

impl Drop for TaskHandle {
    fn drop(&mut self) {
        // SAFETY: `task` is a port right this handle solely owns; releasing it once is correct.
        unsafe {
            mach::mach_port_deallocate(mach::mach_task_self(), self.task);
        }
    }
}

/// Acquire the control port for `pid` and bind it to the incarnation that port currently names.
/// The audit token is read before returning, so nothing is written to the target and the pid is
/// confirmed to match the port.
pub fn bind(pid: pid_t) -> Result<TaskHandle, LoaderError> {
    let task = task_for_pid(pid)?;
    // Own the port immediately so it is released on any early return below.
    let mut handle = TaskHandle { task, incarnation: Incarnation { pid, pidversion: 0 } };
    handle.incarnation = read_incarnation(task)?;
    if handle.incarnation.pid != pid {
        return Err(LoaderError::IncarnationMismatch {
            expected: Incarnation { pid, pidversion: handle.incarnation.pidversion },
            found: handle.incarnation,
        });
    }
    Ok(handle)
}

/// Acquire the control port for `pid` and require it to name `expected`. Used after a session has
/// recorded an incarnation, so a pid reused between commands is refused rather than inspected.
pub fn rebind(pid: pid_t, expected: Incarnation) -> Result<TaskHandle, LoaderError> {
    let handle = bind(pid)?;
    if handle.incarnation == expected {
        Ok(handle)
    } else {
        Err(LoaderError::IncarnationMismatch { expected, found: handle.incarnation })
    }
}

fn task_for_pid(pid: pid_t) -> Result<mach_port_t, LoaderError> {
    let mut task: mach_port_t = 0;
    // SAFETY: `task` is a valid out pointer; the trap writes the control port on success.
    let kr = unsafe { mach::task_for_pid(mach::mach_task_self(), pid, &mut task) };
    if kr == KERN_SUCCESS { Ok(task) } else { Err(LoaderError::TaskUnavailable(kr)) }
}

fn read_incarnation(task: mach_port_t) -> Result<Incarnation, LoaderError> {
    let mut token = AuditToken { val: [0; 8] };
    let mut count = TASK_AUDIT_TOKEN_COUNT;
    // SAFETY: `token` holds eight words and `count` is initialized to that length, as task_info
    // requires for the audit-token flavor.
    let kr = unsafe {
        mach::task_info(task, TASK_AUDIT_TOKEN, token.val.as_mut_ptr().cast(), &mut count)
    };
    if kr != KERN_SUCCESS {
        return Err(LoaderError::AuditTokenUnavailable(kr));
    }
    Ok(Incarnation { pid: token.val[5] as pid_t, pidversion: token.val[7] })
}
