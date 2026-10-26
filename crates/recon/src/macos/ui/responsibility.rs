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
use serde_json::json;

use super::child_output::OutputPipes;
use super::{forwarding, private};
use crate::error::{ErrorCode, Failure};
use crate::macos::ui::child_output;

/// Marks the re-executed child. Its value is the process ID of the `recon` that spawned it, so a
/// marker some other launcher set cannot make Recon believe it has already re-executed.
const MARKER: &str = "RECON_RESPONSIBILITY_DISCLAIMED";

type ResponsibleForPid = unsafe extern "C" fn(pid_t) -> pid_t;
type SetDisclaim = unsafe extern "C" fn(*mut posix_spawnattr_t, c_int) -> c_int;

pub(super) enum ResponsibilityError {
    BeforeSpawn(Failure),
    AfterSpawn(Failure),
}

impl ResponsibilityError {
    pub(super) fn into_failure(self) -> Failure {
        match self {
            Self::BeforeSpawn(failure) | Self::AfterSpawn(failure) => failure,
        }
    }
}

impl From<Failure> for ResponsibilityError {
    fn from(failure: Failure) -> Self {
        Self::BeforeSpawn(failure)
    }
}

/// A post-spawn failure must be the final response, including for context.
pub(super) fn take_responsibility() -> Result<(), ResponsibilityError> {
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
        return Err(unavailable(
            Stage::Verify,
            None,
            None,
            "recon is still not responsible for its own privacy permissions after re-executing \
             itself",
        )
        .into());
    }
    relaunch_disclaimed()
}

/// Whether the marker in the environment is the one this process's parent set for it.
fn relaunched_by(marker: Option<&str>, parent: pid_t) -> bool {
    marker.and_then(|value| value.parse::<pid_t>().ok()) == Some(parent)
}

fn responsible_for_self() -> Result<bool, Failure> {
    // SAFETY: `ResponsibleForPid` is the signature of the responsibility lookup.
    let responsible_for: ResponsibleForPid =
        unsafe { private::function(c"responsibility_get_pid_responsible_for_pid") }
            .ok_or_else(|| missing("responsibility_get_pid_responsible_for_pid"))?;
    // SAFETY: reading this process's own ID cannot fail.
    let own = unsafe { libc::getpid() };
    // SAFETY: the lookup takes any process ID and reports the process macOS holds responsible.
    Ok(unsafe { responsible_for(own) } == own)
}

fn relaunch_disclaimed() -> Result<(), ResponsibilityError> {
    // SAFETY: `SetDisclaim` is the signature of the disclaim spawn-attribute setter.
    let set_disclaim: SetDisclaim =
        unsafe { private::function(c"responsibility_spawnattrs_setdisclaim") }
            .ok_or_else(|| missing("responsibility_spawnattrs_setdisclaim"))?;
    let executable = env::current_exe().map_err(|error| {
        unavailable(
            Stage::Spawn,
            None,
            Some(error.to_string()),
            format!("cannot find the running recon executable: {error}"),
        )
    })?;

    let program = argument(executable.as_os_str().as_bytes())?;
    let arguments = env::args_os()
        .map(|argument_| argument(argument_.as_os_str().as_bytes()))
        .collect::<Result<Vec<_>, _>>()?;
    let environment = disclaimed_environment()?;
    let pipes = OutputPipes::new().map_err(spawn_failure)?;

    let (child, forwarder, streams) = {
        // Holding the stop signals until the child exists means none is lost or mishandled.
        // Holding them before the readers start means the readers hold them too, so a stop
        // signal is handled by the thread that owns the child rather than by one reading a pipe.
        let held = forwarding::hold();
        // A reader-start failure must not leave a child running after the command reports failure.
        let readers = pipes.readers().map_err(drain_failure)?;
        let child = match spawn(
            &program,
            &arguments,
            &environment,
            set_disclaim,
            held.launcher_mask(),
            readers.actions(),
        ) {
            Ok(child) => child,
            Err(failure) => {
                // Nothing took the write ends, so releasing them ends both readers.
                readers.release();
                return Err(failure.into());
            }
        };
        let forwarder = held.forward_to(child);
        (child, forwarder, readers.drain())
    };

    let status = forwarding::wait_for(child);
    forwarder.stop();
    let status = status.map_err(|error| ResponsibilityError::AfterSpawn(wait_failure(error)))?;
    // On a failed wait the reader handles are dropped, not joined: an unconfirmed child may
    // still be alive. The caller emits the final failure and exits without signalling its PID.
    let captured = streams.collect().map_err(output_failure)?;
    captured.forward().map_err(|failure| match failure {
        child_output::ForwardFailure::Stdout(error) => output_failure(error),
        child_output::ForwardFailure::Stderr => std::process::exit(1),
    })?;
    forwarding::exit_like(status)
}

fn spawn(
    program: &CString,
    arguments: &[CString],
    environment: &[CString],
    set_disclaim: SetDisclaim,
    launcher_mask: &libc::sigset_t,
    actions: &libc::posix_spawn_file_actions_t,
) -> Result<pid_t, Failure> {
    let argv = pointers(arguments);
    let envp = pointers(environment);
    let mut child: pid_t = 0;

    let mut attributes = SpawnAttributes::new()?;
    attributes.start_with_mask(launcher_mask)?;
    attributes.disclaim(set_disclaim)?;

    // SAFETY: pointer vectors are null terminated and all spawn configuration outlives the call.
    let spawned = unsafe {
        libc::posix_spawn(
            &mut child,
            program.as_ptr(),
            actions,
            &attributes.0,
            argv.as_ptr(),
            envp.as_ptr(),
        )
    };
    if spawned != 0 {
        return Err(spawn_failure(io::Error::from_raw_os_error(spawned)));
    }
    Ok(child)
}

/// A reader that cannot start is a local resource failure rather than anything macOS said about
/// Recon's identity, and it happens before a child exists, so it stays reportable.
fn drain_failure(error: io::Error) -> Failure {
    Failure::internal(format!("cannot drain child output: {error}"))
}

fn output_failure(error: io::Error) -> ResponsibilityError {
    ResponsibilityError::AfterSpawn(Failure::internal(format!(
        "cannot collect child output: {error}"
    )))
}

/// Spawn attributes, destroyed when they go out of scope so that no failure path leaks them.
struct SpawnAttributes(posix_spawnattr_t);

impl SpawnAttributes {
    fn new() -> Result<Self, Failure> {
        let mut attributes: posix_spawnattr_t = ptr::null_mut();
        // SAFETY: `attributes` is a live local, taken over by the returned value.
        let initialized = unsafe { libc::posix_spawnattr_init(&mut attributes) };
        if initialized != 0 {
            return Err(spawn_failure(io::Error::from_raw_os_error(initialized)));
        }
        Ok(Self(attributes))
    }

    /// Starts the child with the mask the launcher ran with, rather than the one Recon holds
    /// while it installs its handlers, and without unblocking what the launcher blocked.
    fn start_with_mask(&mut self, mask: &libc::sigset_t) -> Result<(), Failure> {
        // SAFETY: the attributes are initialized and the mask is live.
        let masked = unsafe { libc::posix_spawnattr_setsigmask(&mut self.0, mask) };
        if masked != 0 {
            return Err(spawn_failure(io::Error::from_raw_os_error(masked)));
        }
        // SAFETY: the attributes are initialized.
        let flagged = unsafe {
            libc::posix_spawnattr_setflags(&mut self.0, libc::POSIX_SPAWN_SETSIGMASK as c_short)
        };
        if flagged != 0 {
            return Err(spawn_failure(io::Error::from_raw_os_error(flagged)));
        }
        Ok(())
    }

    fn disclaim(&mut self, set_disclaim: SetDisclaim) -> Result<(), Failure> {
        // SAFETY: the private setter takes initialized spawn attributes.
        let disclaimed = unsafe { set_disclaim(&mut self.0, 1) };
        if disclaimed != 0 {
            let error = io::Error::from_raw_os_error(disclaimed);
            return Err(unavailable(
                Stage::Disclaim,
                None,
                Some(error.to_string()),
                format!("cannot disclaim recon's responsibility: {error}"),
            ));
        }
        Ok(())
    }
}

impl Drop for SpawnAttributes {
    fn drop(&mut self) {
        // SAFETY: initialized in `new` and not used after this.
        unsafe { libc::posix_spawnattr_destroy(&mut self.0) };
    }
}

fn disclaimed_environment() -> Result<Vec<CString>, Failure> {
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

fn argument(bytes: &[u8]) -> Result<CString, Failure> {
    CString::new(bytes).map_err(|_| {
        unavailable(
            Stage::Spawn,
            None,
            None,
            "cannot re-execute recon: an argument contains a null byte",
        )
    })
}

fn pointers(strings: &[CString]) -> Vec<*mut c_char> {
    let mut pointers: Vec<*mut c_char> =
        strings.iter().map(|string| string.as_ptr().cast_mut()).collect();
    pointers.push(ptr::null_mut());
    pointers
}

/// Where Recon got to before it gave up on owning its privacy identity. An agent recovers
/// differently from each: a missing symbol is this macOS version, a failed spawn is this machine.
#[derive(Clone, Copy)]
enum Stage {
    Lookup,
    Spawn,
    Disclaim,
    Wait,
    Verify,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Stage::Lookup => "lookup",
            Stage::Spawn => "spawn",
            Stage::Disclaim => "disclaim",
            Stage::Wait => "wait",
            Stage::Verify => "verify",
        }
    }
}

fn spawn_failure(error: io::Error) -> Failure {
    unavailable(
        Stage::Spawn,
        None,
        Some(error.to_string()),
        format!("cannot re-execute recon as its own responsible process: {error}"),
    )
}

fn wait_failure(error: io::Error) -> Failure {
    unavailable(
        Stage::Wait,
        None,
        Some(error.to_string()),
        format!("cannot wait for the disclaimed recon process: {error}"),
    )
}

fn missing(symbol: &str) -> Failure {
    unavailable(
        Stage::Lookup,
        Some(symbol),
        None,
        format!(
            "recon cannot take responsibility for its own privacy permissions on this macOS \
             version ({symbol} not found)"
        ),
    )
}

fn unavailable(
    stage: Stage,
    symbol: Option<&str>,
    detail: Option<String>,
    message: impl Into<String>,
) -> Failure {
    Failure::new(
        ErrorCode::IdentityUnavailable,
        message,
        json!({"stage": stage.name(), "symbol": symbol, "detail": detail}),
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
