//! macOS attributes a privacy request to the *responsible* process, which for a command-line tool
//! is by default the terminal, editor, or harness that launched it. Recon re-executes itself once
//! with the private disclaim spawn attribute so that its grants belong to the `recon` executable
//! and behave the same under every launcher.

use std::env;
use std::ffi::{CString, c_char, c_int};
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::process::exit;
use std::ptr;

use libc::{pid_t, posix_spawnattr_t};

use super::private;

/// Marks the re-executed child, so that responsibility is taken exactly once.
const MARKER: &str = "RECON_RESPONSIBILITY_DISCLAIMED";

type ResponsibleForPid = unsafe extern "C" fn(pid_t) -> pid_t;
type SetDisclaim = unsafe extern "C" fn(*mut posix_spawnattr_t, c_int) -> c_int;

/// Makes `recon` responsible for its own privacy requests, re-executing it once if it is not.
///
/// When a re-execution happens this never returns: the parent exits with the child's status.
pub fn take_responsibility() -> Result<(), String> {
    let relaunched = env::var_os(MARKER).is_some();
    if relaunched {
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

    let child = spawn(&program, &arguments, &environment, set_disclaim)?;
    exit(wait_for(child)?)
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
    if unsafe { libc::posix_spawnattr_init(&mut attributes) } != 0 {
        return Err(spawn_failure(io::Error::last_os_error()));
    }
    // SAFETY: the attributes were just initialized.
    let disclaimed = unsafe { set_disclaim(&mut attributes, 1) };
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

fn wait_for(child: pid_t) -> Result<i32, String> {
    let mut status: c_int = 0;
    // SAFETY: `child` is the process just spawned, and `status` is a live local.
    while unsafe { libc::waitpid(child, &mut status, 0) } < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(format!("cannot wait for the disclaimed recon process: {error}"));
        }
    }
    if libc::WIFSIGNALED(status) {
        return Ok(128 + libc::WTERMSIG(status));
    }
    Ok(libc::WEXITSTATUS(status))
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
    environment.push(argument(format!("{MARKER}=1").as_bytes())?);
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
