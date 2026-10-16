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

/// The forwarded signals, held back until there is a child to forward them to.
///
/// Blocking them closes the window between spawning the child and installing the handler: a
/// signal that arrives in between stays pending, and is delivered to the handler once the mask
/// is restored, which happens when this is dropped.
pub struct Held(libc::sigset_t);

pub fn hold() -> Held {
    let mut forwarded = no_signals();
    let mut held = no_signals();
    // SAFETY: every set is a live local.
    unsafe {
        for signal in FORWARDED {
            libc::sigaddset(&mut forwarded, signal);
        }
        libc::sigprocmask(libc::SIG_BLOCK, &forwarded, &mut held);
    }
    Held(held)
}

impl Held {
    /// Sends the forwarded signals to `child` from now on.
    pub fn forward_to(&self, child: pid_t) {
        CHILD.store(child, Ordering::Relaxed);
        let mut action = empty_action();
        action.sa_sigaction = forward_to_child as *const () as libc::sighandler_t;
        for signal in FORWARDED {
            // SAFETY: the handler is async-signal-safe and `action` is a live local.
            unsafe { libc::sigaction(signal, &action, ptr::null_mut()) };
        }
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        // SAFETY: `self.0` is the mask this process ran with before the signals were blocked.
        unsafe { libc::sigprocmask(libc::SIG_SETMASK, &self.0, ptr::null_mut()) };
    }
}

/// An empty signal set, which a spawned child also needs so that it does not inherit the mask its
/// parent holds while installing handlers.
pub fn no_signals() -> libc::sigset_t {
    // SAFETY: `sigemptyset` initializes the set it is given.
    unsafe {
        let mut set = mem::zeroed::<libc::sigset_t>();
        libc::sigemptyset(&mut set);
        set
    }
}

pub fn wait_for(child: pid_t) -> Result<c_int, String> {
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
pub fn exit_like(status: c_int) -> ! {
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
    // process; all three take live locals or a signal number.
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
        libc::sigemptyset(&mut action.sa_mask);
        action
    }
}
