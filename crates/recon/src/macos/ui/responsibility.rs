//! macOS attributes a privacy request to the *responsible* process, which for a command-line tool
//! is by default the terminal, editor, or harness that launched it. Recon re-executes itself once
//! with the private disclaim spawn attribute so that its grants belong to the `recon` executable
//! and behave the same under every launcher.

use std::env;
use std::ffi::{CString, OsStr, c_char, c_int, c_short};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::ptr;

use libc::{pid_t, posix_spawnattr_t};

use super::{forwarding, private};

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

    let child = {
        // Holding the stop signals until the child exists means none is lost or mishandled.
        let held = forwarding::hold();
        let child = spawn(&program, &arguments, &environment, set_disclaim)?;
        held.forward_to(child);
        child
    };
    forwarding::exit_like(forwarding::wait_for(child)?)
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
        libc::posix_spawnattr_setsigmask(&mut attributes, &forwarding::no_signals());
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
