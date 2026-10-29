//! What `NSWorkspace` knows: which applications are running, and how to launch one.

use block2::RcBlock;
use objc2_app_kit::{NSRunningApplication, NSWorkspace, NSWorkspaceOpenConfiguration};
use objc2_foundation::{NSError, NSString, NSURL};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Instant;

use crate::application::Pid;
use crate::error::{ErrorCode, Failure};

/// The bundle path and process ID of every running application that has an identifiable bundle.
pub(super) fn running_applications() -> Vec<(PathBuf, Pid)> {
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter_map(|running| {
            let pid = Pid(u32::try_from(running.processIdentifier()).ok()?);
            let path = PathBuf::from(running.bundleURL()?.path()?.to_string());
            let path = path.canonicalize().ok()?;
            Some((path, pid))
        })
        .collect()
}

/// Why macOS did not open an application, as a `LAUNCH_FAILED` reason reports it.
enum Refusal {
    Refused(String),
    NoProcessId,
    NotOpened,
}

impl Refusal {
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
        }
    }
}

/// Opens and activates the application at exactly `path`, reusing an instance already running
/// from that path and never substituting a running copy from another location.
///
/// # Errors
///
/// [`ErrorCode::LaunchFailed`], naming in its details which step did not happen.
pub(super) fn launch(path: &Path) -> Result<Pid, Failure> {
    launch_with_activation(path, true, None)
}

/// Starts exactly `path` without requesting activation or substituting another running copy.
pub(super) fn launch_background(path: &Path, deadline: Instant) -> Result<Pid, Failure> {
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
    let Some(path_string) = path.to_str() else {
        return Err(launch_failure(
            path,
            "path-not-utf8",
            None,
            format!("application path is not UTF-8: {}", path.display()),
        ));
    };
    let url = NSURL::fileURLWithPath(&NSString::from_str(path_string));
    let configuration = NSWorkspaceOpenConfiguration::configuration();
    configuration.setActivates(activates);
    configuration.setCreatesNewApplicationInstance(false);
    configuration.setAllowsRunningApplicationSubstitution(false);

    let (sender, receiver) = mpsc::sync_channel(1);
    let completion =
        RcBlock::new(move |running: *mut NSRunningApplication, error: *mut NSError| {
            // SAFETY: AppKit passes each argument as null or as an object that stays valid for
            // the duration of the completion handler.
            let response = unsafe {
                if let Some(error) = error.as_ref() {
                    Err(Refusal::Refused(error.localizedDescription().to_string()))
                } else if let Some(running) = running.as_ref() {
                    u32::try_from(running.processIdentifier())
                        .map(Pid)
                        .map_err(|_| Refusal::NoProcessId)
                } else {
                    Err(Refusal::NotOpened)
                }
            };
            // The receiver is gone only after `launch` has returned, when no one is left to tell.
            let _ = sender.send(response);
        });
    NSWorkspace::sharedWorkspace().openApplicationAtURL_configuration_completionHandler(
        &url,
        &configuration,
        Some(&completion),
    );
    let opened = match deadline {
        Some(limit) => receiver
            .recv_timeout(limit.saturating_duration_since(Instant::now()))
            .map_err(|error| match error {
                RecvTimeoutError::Timeout => background_launch_timeout(path),
                RecvTimeoutError::Disconnected => launch_failure(
                    path,
                    "callback-failed",
                    Some(error.to_string()),
                    format!("macOS launch callback failed: {error}"),
                ),
            })?,
        None => receiver.recv().map_err(|error| {
            launch_failure(
                path,
                "callback-failed",
                Some(error.to_string()),
                format!("macOS launch callback failed: {error}"),
            )
        })?,
    };
    opened.map_err(|refusal| {
        let (reason, detail, sentence) = refusal.parts();
        launch_failure(
            path,
            reason,
            detail,
            format!("cannot launch {}: {sentence}", path.display()),
        )
    })
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
    use std::time::{Duration, Instant};

    #[test]
    fn each_refusal_names_the_step_that_did_not_happen() {
        let reasons: Vec<&str> = [
            Refusal::Refused("The application is damaged".to_owned()),
            Refusal::NoProcessId,
            Refusal::NotOpened,
        ]
        .into_iter()
        .map(|refusal| refusal.parts().0)
        .collect();

        assert_eq!(reasons, ["refused", "no-process-id", "not-opened"]);
    }

    #[test]
    fn a_refusal_carries_what_macos_said_as_structured_detail() {
        let (reason, detail, sentence) =
            Refusal::Refused("The application is damaged".to_owned()).parts();

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
