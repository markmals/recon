//! macOS attributes a privacy request to the *responsible* process, which for a command-line tool
//! is by default the terminal, editor, or harness that launched it. Recon re-executes itself once
//! with the private disclaim spawn attribute so that its grants belong to the `recon` executable
//! and behave the same under every launcher.

use std::env;
use std::ffi::{CString, OsStr, c_char, c_int, c_short};
use std::io;
use std::mem;
use std::os::unix::ffi::OsStrExt;
use std::process::exit;
use std::ptr;
use std::sync::atomic::{AtomicI32, Ordering};

use libc::{pid_t, posix_spawnattr_t};

use super::private;

/// Marks the re-executed child. Its value is the process ID of the `recon` that spawned it, so a
/// marker some other launcher set cannot make Recon believe it has already re-executed.
const MARKER: &str = "RECON_RESPONSIBILITY_DISCLAIMED";

type ResponsibleForPid = unsafe extern "C" fn(pid_t) -> pid_t;
type SetDisclaim = unsafe extern "C" fn(*mut posix_spawnattr_t, c_int) -> c_int;

/// Makes `recon` responsible for its own privacy requests, re-executing it once if it is not.
///
/// When a re-execution happens this never returns: the parent exits with the child's status.
pub fn take_responsibility() -> Result<(), String> {
    let marker = env::var_os(MARKER);
    // SAFETY: reading this process's parent cannot fail.
    let parent = unsafe { libc::getppid() };
    let relaunched = relaunched_by(marker.as_deref().and_then(OsStr::to_str), parent);
    if marker.is_some() {
        // SAFETY: Recon is single threaded at startup, before anything else reads the environment.
        unsafe { env::remove_var(MARKER) };
    }

    if responsible_for_self()? {
        return Ok(());
    }
    if relaunched {
        return Err(
            "recon is still not responsible for its own privacy permissions after re-executing \
             itself"
                .to_owned(),
        );
    }
    relaunch_disclaimed()
}

/// Whether the marker in the environment is the one this process's parent set for it.
fn relaunched_by(marker: Option<&str>, parent: pid_t) -> bool {
    marker.and_then(|value| value.parse::<pid_t>().ok()) == Some(parent)
}

fn responsible_for_self() -> Result<bool, String> {
    let responsible_for: ResponsibleForPid =
        private::function(c"responsibility_get_pid_responsible_for_pid")
            .ok_or_else(|| unavailable("responsibility_get_pid_responsible_for_pid"))?;
    // SAFETY: the lookup reports the process macOS holds responsible for the given one.
    let own = unsafe { libc::getpid() };
    Ok(unsafe { responsible_for(own) } == own)
}

fn relaunch_disclaimed() -> Result<(), String> {
    let set_disclaim: SetDisclaim = private::function(c"responsibility_spawnattrs_setdisclaim")
        .ok_or_else(|| unavailable("responsibility_spawnattrs_setdisclaim"))?;
    let executable = env::current_exe()
        .map_err(|error| format!("cannot find the running recon executable: {error}"))?;

    let program = argument(executable.as_os_str().as_bytes())?;
    let arguments = env::args_os()
        .map(|argument_| argument(argument_.as_os_str().as_bytes()))
        .collect::<Result<Vec<_>, _>>()?;
    let environment = disclaimed_environment()?;

    // Blocking first closes the window between the spawn and the handler: a signal that arrives
    // in between stays pending and is delivered, to the installed handler, once both are done.
    let held = block_forwarded();
    let spawned = spawn(&program, &arguments, &environment, set_disclaim);
    if let Ok(child) = spawned {
        CHILD.store(child, Ordering::Relaxed);
        install_forwarding();
    }
    restore_mask(&held);

    let child = spawned?;
    exit_like(wait_for(child)?)
}

fn spawn(
    program: &CString,
    arguments: &[CString],
    environment: &[CString],
    set_disclaim: SetDisclaim,
) -> Result<pid_t, String> {
    let argv = pointers(arguments);
    let envp = pointers(environment);
    let mut attributes: posix_spawnattr_t = ptr::null_mut();
    let mut child: pid_t = 0;

    // SAFETY: `attributes` is a live local, destroyed below.
    let initialized = unsafe { libc::posix_spawnattr_init(&mut attributes) };
    if initialized != 0 {
        return Err(spawn_failure(io::Error::from_raw_os_error(initialized)));
    }
    // SAFETY: the attributes were just initialized; the child must not inherit the mask the
    // parent holds while it installs its handlers, or it could never be stopped.
    let disclaimed = unsafe {
        libc::posix_spawnattr_setsigmask(&mut attributes, &empty_signals());
        libc::posix_spawnattr_setflags(&mut attributes, libc::POSIX_SPAWN_SETSIGMASK as c_short);
        set_disclaim(&mut attributes, 1)
    };
    let spawned = if disclaimed == 0 {
        // SAFETY: both pointer vectors are null terminated and outlive the call, and without file
        // actions the child inherits Recon's standard streams.
        unsafe {
            libc::posix_spawn(
                &mut child,
                program.as_ptr(),
                ptr::null(),
                &attributes,
                argv.as_ptr(),
                envp.as_ptr(),
            )
        }
    } else {
        0
    };
    // SAFETY: the attributes were initialized above and are not used afterwards.
    unsafe { libc::posix_spawnattr_destroy(&mut attributes) };

    if disclaimed != 0 {
        return Err(format!(
            "cannot disclaim recon's responsibility: {}",
            io::Error::from_raw_os_error(disclaimed)
        ));
    }
    if spawned != 0 {
        return Err(spawn_failure(io::Error::from_raw_os_error(spawned)));
    }
    Ok(child)
}

fn wait_for(child: pid_t) -> Result<c_int, String> {
    let mut status: c_int = 0;
    // SAFETY: `child` is the process just spawned, and `status` is a live local.
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
fn exit_like(status: c_int) -> ! {
    if libc::WIFSIGNALED(status) {
        let signal = libc::WTERMSIG(status);
        reraise(signal);
        // Only reached if the signal did not end this process after all.
        exit(128 + signal);
    }
    exit(libc::WEXITSTATUS(status));
}

fn reraise(signal: c_int) {
    let mut action: libc::sigaction = empty_action();
    action.sa_sigaction = libc::SIG_DFL;
    let mut only = empty_signals();
    // SAFETY: restores the signal's default disposition, unblocks it, and sends it to this
    // process; all three take live locals or a signal number.
    unsafe {
        libc::sigaction(signal, &action, ptr::null_mut());
        libc::sigaddset(&mut only, signal);
        libc::sigprocmask(libc::SIG_UNBLOCK, &only, ptr::null_mut());
        libc::raise(signal);
    }
}

/// Signals that stop a command. The parent forwards them, so stopping `recon` stops the
/// observation the disclaimed child is doing.
const FORWARDED: [c_int; 4] = [libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM];

/// The disclaimed child, which is all the signal handler is allowed to read.
static CHILD: AtomicI32 = AtomicI32::new(0);

extern "C" fn forward_to_child(signal: c_int) {
    let child = CHILD.load(Ordering::Relaxed);
    if child > 0 {
        // SAFETY: `kill` is async-signal-safe, and the handler touches nothing else.
        unsafe { libc::kill(child, signal) };
    }
}

fn install_forwarding() {
    let mut action: libc::sigaction = empty_action();
    action.sa_sigaction = forward_to_child as *const () as libc::sighandler_t;
    for signal in FORWARDED {
        // SAFETY: the handler is async-signal-safe and `action` is a live local.
        unsafe { libc::sigaction(signal, &action, ptr::null_mut()) };
    }
}

fn block_forwarded() -> libc::sigset_t {
    let mut forwarded = empty_signals();
    let mut held = empty_signals();
    // SAFETY: every set is a live local.
    unsafe {
        for signal in FORWARDED {
            libc::sigaddset(&mut forwarded, signal);
        }
        libc::sigprocmask(libc::SIG_BLOCK, &forwarded, &mut held);
    }
    held
}

fn restore_mask(held: &libc::sigset_t) {
    // SAFETY: `held` is the mask this process ran with before blocking.
    unsafe { libc::sigprocmask(libc::SIG_SETMASK, held, ptr::null_mut()) };
}

fn empty_signals() -> libc::sigset_t {
    // SAFETY: `sigemptyset` initializes the set it is given.
    unsafe {
        let mut set = mem::zeroed::<libc::sigset_t>();
        libc::sigemptyset(&mut set);
        set
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

fn disclaimed_environment() -> Result<Vec<CString>, String> {
    let mut environment = env::vars_os()
        .map(|(name, value)| {
            let mut entry = name.as_bytes().to_vec();
            entry.push(b'=');
            entry.extend_from_slice(value.as_bytes());
            argument(&entry)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // SAFETY: reading this process's own ID cannot fail; the child matches it against its parent.
    let own = unsafe { libc::getpid() };
    environment.push(argument(format!("{MARKER}={own}").as_bytes())?);
    Ok(environment)
}

fn argument(bytes: &[u8]) -> Result<CString, String> {
    CString::new(bytes)
        .map_err(|_| "cannot re-execute recon: an argument contains a null byte".to_owned())
}

fn pointers(strings: &[CString]) -> Vec<*mut c_char> {
    let mut pointers: Vec<*mut c_char> =
        strings.iter().map(|string| string.as_ptr().cast_mut()).collect();
    pointers.push(ptr::null_mut());
    pointers
}

fn spawn_failure(error: io::Error) -> String {
    format!("cannot re-execute recon as its own responsible process: {error}")
}

fn unavailable(symbol: &str) -> String {
    format!(
        "recon cannot take responsibility for its own privacy permissions on this macOS version \
         ({symbol} not found)"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_marker_this_parent_set_means_recon_already_re_executed() {
        assert!(relaunched_by(Some("412"), 412));
        assert!(!relaunched_by(Some("412"), 413));
    }

    #[test]
    fn a_marker_another_launcher_set_is_ignored() {
        assert!(!relaunched_by(None, 412));
        assert!(!relaunched_by(Some("1"), 412));
        assert!(!relaunched_by(Some(""), 412));
        assert!(!relaunched_by(Some("yes"), 412));
        assert!(!relaunched_by(Some("412 "), 412));
    }
}
