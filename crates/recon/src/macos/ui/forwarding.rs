//! Passing a caller's intent through to the disclaimed child: the signals that stop a command go
//! to the child, and the way the child ended becomes the way `recon` ends.

use std::ffi::c_int;
use std::io;
use std::mem;
use std::process::exit;
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};

use libc::pid_t;

/// Signals that stop a command. Forwarding them means stopping `recon` also stops the observation
/// the child is doing, rather than leaving it running without its parent.
const FORWARDED: [c_int; 4] = [libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM];

/// The disclaimed child, which is all the signal handler is allowed to read.
static CHILD: AtomicI32 = AtomicI32::new(0);

/// The stop signals Recon forwards, held back until there is a child to forward them to.
///
/// Blocking them closes the window between spawning the child and installing the handler: a
/// signal that arrives in between stays pending, and is delivered to the handler once the mask
/// is restored, which happens when this is dropped.
pub(super) struct Held {
    previous: libc::sigset_t,
    forwarded: Vec<c_int>,
}

pub(super) fn hold() -> Held {
    let forwarded = forwardable(FORWARDED.map(is_ignored));
    let mut blocked = no_signals();
    let mut previous = no_signals();
    // SAFETY: every set is a live local.
    unsafe {
        for signal in &forwarded {
            debug_check(libc::sigaddset(&mut blocked, *signal));
        }
        debug_check(libc::sigprocmask(libc::SIG_BLOCK, &blocked, &mut previous));
    }
    Held { previous, forwarded }
}

/// Which stop signals to forward, given which ones the launcher set to be ignored. An ignored
/// signal stays ignored by both processes: the child inherits `SIG_IGN` across the spawn, so a
/// parent that handled it would react to something its child is deaf to.
fn forwardable(ignored: [bool; FORWARDED.len()]) -> Vec<c_int> {
    FORWARDED
        .iter()
        .zip(ignored)
        .filter(|(_, ignored)| !ignored)
        .map(|(signal, _)| *signal)
        .collect()
}

fn is_ignored(signal: c_int) -> bool {
    let mut current = empty_action();
    // SAFETY: a null new action only reads the signal's current disposition.
    debug_check(unsafe { libc::sigaction(signal, ptr::null(), &mut current) });
    current.sa_sigaction == libc::SIG_IGN
}

impl Held {
    /// The mask the launcher ran with, which the child must start with: Recon neither unblocks
    /// what the launcher blocked nor passes on the block it holds while installing its handlers.
    pub(super) fn launcher_mask(&self) -> &libc::sigset_t {
        &self.previous
    }

    /// Sends the stop signals to `child` until the returned forwarding is stopped.
    pub(super) fn forward_to(&self, child: pid_t) -> Forwarding {
        CHILD.store(child, Ordering::Relaxed);
        let mut action = empty_action();
        action.sa_sigaction = forward_to_child as *const () as libc::sighandler_t;
        let previous = self
            .forwarded
            .iter()
            .map(|signal| {
                let mut replaced = empty_action();
                // SAFETY: the handler is async-signal-safe, and both actions are live locals.
                debug_check(unsafe { libc::sigaction(*signal, &action, &mut replaced) });
                (*signal, replaced)
            })
            .collect();
        Forwarding { mask: self.previous, previous }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        // SAFETY: `previous` is the mask this process ran with before the signals were blocked.
        debug_check(unsafe {
            libc::sigprocmask(libc::SIG_SETMASK, &self.previous, ptr::null_mut())
        });
    }
}

/// The stop signals while they belong to the child.
pub(super) struct Forwarding {
    mask: libc::sigset_t,
    previous: Vec<(c_int, libc::sigaction)>,
}

impl Forwarding {
    /// Stops forwarding once the child is gone: blocks the stop signals so none is handled while
    /// the handler is being taken down, forgets the child, puts each disposition back, and then
    /// restores the launcher's mask.
    pub(super) fn stop(self) {
        let mut blocked = no_signals();
        // SAFETY: every set and action here is a live local.
        unsafe {
            for (signal, _) in &self.previous {
                debug_check(libc::sigaddset(&mut blocked, *signal));
            }
            debug_check(libc::sigprocmask(libc::SIG_BLOCK, &blocked, ptr::null_mut()));
        }
        CHILD.store(0, Ordering::Relaxed);
        // SAFETY: each action is the one this process had before forwarding began.
        unsafe {
            for (signal, previous) in &self.previous {
                debug_check(libc::sigaction(*signal, previous, ptr::null_mut()));
            }
            debug_check(libc::sigprocmask(libc::SIG_SETMASK, &self.mask, ptr::null_mut()));
        }
    }
}

fn no_signals() -> libc::sigset_t {
    // SAFETY: `sigemptyset` initializes the set it is given.
    unsafe {
        let mut set = mem::zeroed::<libc::sigset_t>();
        debug_check(libc::sigemptyset(&mut set));
        set
    }
}

pub(super) fn wait_for(child: pid_t) -> Result<c_int, String> {
    let mut status: c_int = 0;
    // SAFETY: `child` is the process this one spawned, and `status` is a live local.
    while unsafe { libc::waitpid(child, &mut status, 0) } < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!("cannot wait for the disclaimed recon process: {error}"));
        }
    }
    Ok(status)
}

/// Ends `recon` the way its child ended, so a caller that stopped the observation sees the signal
/// it sent rather than an ordinary exit code.
pub(super) fn exit_like(status: c_int) -> ! {
    if libc::WIFSIGNALED(status) {
        let signal = libc::WTERMSIG(status);
        reraise(signal);
        // Only reached if the signal did not end this process after all.
        exit(128 + signal);
    }
    exit(libc::WEXITSTATUS(status));
}

extern "C" fn forward_to_child(signal: c_int) {
    let child = CHILD.load(Ordering::Relaxed);
    if child > 0 {
        // SAFETY: `kill` is async-signal-safe, and the handler touches nothing else.
        unsafe { libc::kill(child, signal) };
    }
}

fn reraise(signal: c_int) {
    let mut action = empty_action();
    action.sa_sigaction = libc::SIG_DFL;
    let mut only = no_signals();
    // SAFETY: restores the signal's default disposition, unblocks it, and sends it to this
    // process; all three take live locals or a signal number. Their results are not checked:
    // SIGKILL and SIGSTOP refuse a new disposition, and `exit_like` reports the signal anyway.
    unsafe {
        libc::sigaction(signal, &action, ptr::null_mut());
        libc::sigaddset(&mut only, signal);
        libc::sigprocmask(libc::SIG_UNBLOCK, &only, ptr::null_mut());
        libc::raise(signal);
    }
}

fn empty_action() -> libc::sigaction {
    // SAFETY: a zeroed `sigaction` with an emptied mask and no flags.
    unsafe {
        let mut action = mem::zeroed::<libc::sigaction>();
        debug_check(libc::sigemptyset(&mut action.sa_mask));
        action
    }
}

/// The signal calls checked here fail only for an invalid signal number or mask operation, which
/// their constant arguments rule out, so a failure is a programming error.
fn debug_check(result: c_int) {
    debug_assert_eq!(result, 0, "signal call failed: {}", io::Error::last_os_error());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_stop_signal_is_forwarded_when_the_launcher_ignores_none() {
        assert_eq!(forwardable([false; FORWARDED.len()]), FORWARDED.to_vec());
    }

    #[test]
    fn a_signal_the_launcher_ignores_is_not_forwarded() {
        assert_eq!(
            forwardable([true, false, true, false]),
            vec![libc::SIGINT, libc::SIGTERM],
            "nohup-style ignoring of SIGHUP and SIGQUIT leaves the rest forwarded"
        );
        assert!(forwardable([true; FORWARDED.len()]).is_empty());
    }
}
