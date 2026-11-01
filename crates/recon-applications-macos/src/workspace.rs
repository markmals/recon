//! What `NSWorkspace` knows: which applications are running, which one is frontmost, and how to
//! launch one. The framework calls are made from this crate's Swift side; the rules about what
//! they mean are here.

use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::json;

use recon_core::error::{ErrorCode, Failure};
use recon_core::process::Pid;

use crate::swift::{self, Opened};

/// The bundle path and process ID of every running application that has an identifiable bundle.
pub fn running_applications() -> Vec<(PathBuf, Pid)> {
    swift::running_applications()
        .into_iter()
        .filter_map(|(path, pid)| Some((PathBuf::from(path).canonicalize().ok()?, Pid(pid))))
        .collect()
}

/// The process ID of the application macOS currently reports as frontmost, or `None` when it
/// reports none.
///
/// `NSWorkspace` learns of activation changes through run-loop notifications, so a caller that
/// never runs its run loop keeps seeing whichever application was frontmost when it started.
pub fn frontmost_application() -> Option<Pid> {
    swift::frontmost_application().map(Pid)
}

/// Why macOS did not open an application, as a `LAUNCH_FAILED` reason reports it.
enum Refusal {
    Refused(String),
    NoProcessId,
    NotOpened,
    CallbackFailed(String),
}

impl Refusal {
    /// What macOS answered, as the reason an agent branches on.
    fn of(reason: &str, detail: Option<String>) -> Self {
        match reason {
            "refused" => {
                Refusal::Refused(detail.unwrap_or_else(|| "macOS refused the open".to_owned()))
            }
            "no-process-id" => Refusal::NoProcessId,
            _ => Refusal::NotOpened,
        }
    }

    /// The reason an agent branches on, the detail macOS gave, and the sentence a person reads.
    fn parts(self) -> (&'static str, Option<String>, String) {
        match self {
            Refusal::Refused(description) => ("refused", Some(description.clone()), description),
            Refusal::NoProcessId => (
                "no-process-id",
                None,
                "macOS opened an application without a process ID".to_owned(),
            ),
            Refusal::NotOpened => {
                ("not-opened", None, "macOS did not open the application".to_owned())
            }
            Refusal::CallbackFailed(detail) => (
                "callback-failed",
                Some(detail.clone()),
                format!("macOS launch callback failed: {detail}"),
            ),
        }
    }
}

/// Opens and activates the application at exactly `path`, reusing an instance already running
/// from that path and never substituting a running copy from another location.
///
/// # Errors
///
/// [`ErrorCode::LaunchFailed`], naming in its details which step did not happen.
pub fn launch(path: &Path) -> Result<Pid, Failure> {
    launch_with_activation(path, true, None)
}

/// Starts exactly `path` without requesting activation or substituting another running copy.
///
/// # Errors
///
/// [`ErrorCode::InspectionTimeout`] when the open budget expires, and
/// [`ErrorCode::LaunchFailed`] when macOS refused.
pub fn launch_background(path: &Path, deadline: Instant) -> Result<Pid, Failure> {
    launch_with_activation(path, false, Some(deadline))
}

fn launch_with_activation(
    path: &Path,
    activates: bool,
    deadline: Option<Instant>,
) -> Result<Pid, Failure> {
    if deadline.is_some_and(|limit| Instant::now() >= limit) {
        return Err(background_launch_timeout(path));
    }
    if path.to_str().is_none() {
        return Err(launch_failure(
            path,
            "path-not-utf8",
            None,
            format!("application path is not UTF-8: {}", path.display()),
        ));
    }
    // A negative budget is the open that waits for LaunchServices however long it takes.
    let budget = deadline
        .map_or(-1.0, |limit| limit.saturating_duration_since(Instant::now()).as_secs_f64());
    let refusal = match swift::open_application(path, activates, budget) {
        Opened::Pid(pid) => return Ok(Pid(pid)),
        Opened::TimedOut => return Err(background_launch_timeout(path)),
        Opened::Refused { reason, detail } => Refusal::of(&reason, detail),
        Opened::Unreadable(detail) => Refusal::CallbackFailed(detail),
    };
    let (reason, detail, sentence) = refusal.parts();
    Err(launch_failure(
        path,
        reason,
        detail,
        format!("cannot launch {}: {sentence}", path.display()),
    ))
}

fn background_launch_timeout(path: &Path) -> Failure {
    Failure::new(
        ErrorCode::InspectionTimeout,
        format!(
            "the inspection open deadline expired while starting {}; launch may still finish",
            path.display()
        ),
        json!({ "detail": "LaunchServices callback did not complete within the open budget" }),
    )
}

fn launch_failure(path: &Path, reason: &str, detail: Option<String>, message: String) -> Failure {
    Failure::new(
        ErrorCode::LaunchFailed,
        message,
        json!({
            "application": path.display().to_string(),
            "reason": reason,
            "detail": detail,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn each_refusal_names_the_step_that_did_not_happen() {
        let reasons: Vec<&str> = [
            Refusal::Refused("The application is damaged".to_owned()),
            Refusal::NoProcessId,
            Refusal::NotOpened,
            Refusal::CallbackFailed("macOS reported nothing".to_owned()),
        ]
        .into_iter()
        .map(|refusal| refusal.parts().0)
        .collect();

        assert_eq!(reasons, ["refused", "no-process-id", "not-opened", "callback-failed"]);
    }

    #[test]
    fn a_refusal_carries_what_macos_said_as_structured_detail() {
        let (reason, detail, sentence) =
            Refusal::of("refused", Some("The application is damaged".to_owned())).parts();

        assert_eq!(reason, "refused");
        assert_eq!(detail.as_deref(), Some("The application is damaged"));
        assert_eq!(sentence, "The application is damaged");

        let failure = launch_failure(
            Path::new("/Applications/Broken.app"),
            reason,
            detail,
            format!("cannot launch /Applications/Broken.app: {sentence}"),
        );
        assert_eq!(failure.code, ErrorCode::LaunchFailed);
        assert_eq!(
            failure.details.expect("the data needed to recover"),
            json!({
                "application": "/Applications/Broken.app",
                "reason": "refused",
                "detail": "The application is damaged",
            })
        );
    }

    #[test]
    fn expired_session_open_budget_never_starts_a_background_application() {
        let failure = launch_background(
            Path::new("/Applications/Nonexistent.app"),
            Instant::now() - Duration::from_millis(1),
        )
        .expect_err("an expired open must stop before LaunchServices is called");
        assert_eq!(failure.code, ErrorCode::InspectionTimeout);
    }
}
