//! Choosing which running instance of a bundle `recon ui` observes.

use std::path::Path;

use serde_json::json;

use crate::application::Pid;
use crate::error::{ErrorCode, Failure, NotFound};

/// Picks the one running process of a resolved bundle, never guessing between several.
///
/// # Errors
///
/// [`ErrorCode::NotRunning`] when the bundle is not running, [`ErrorCode::NotFound`] when
/// `requested` is not one of its processes, and [`ErrorCode::AmbiguousInstance`] when it has
/// several and none was requested.
pub fn select_instance(
    application_path: &Path,
    running_pids: &[Pid],
    requested: Option<Pid>,
) -> Result<Pid, Failure> {
    let application = application_path.display();
    if running_pids.is_empty() {
        return Err(Failure::new(
            ErrorCode::NotRunning,
            format!("{application} is not running; recon ui does not launch applications"),
            json!({ "application": application.to_string() }),
        ));
    }
    if let Some(pid) = requested {
        if running_pids.contains(&pid) {
            return Ok(pid);
        }
        return Err(Failure::new(
            ErrorCode::NotFound,
            format!("process {pid} is not a running instance of {application}"),
            NotFound {
                kind: "instance",
                path: Some(application.to_string()),
                pid: Some(pid.0),
                running_pids: Some(running_pids),
                ..Default::default()
            }
            .details(),
        ));
    }
    if let [only] = running_pids {
        return Ok(*only);
    }
    let candidates = running_pids.iter().map(Pid::to_string).collect::<Vec<_>>().join(", ");
    Err(Failure::new(
        ErrorCode::AmbiguousInstance,
        format!("{application} has several running instances; choose one with --pid: {candidates}"),
        json!({ "application": application.to_string(), "pids": running_pids }),
    ))
}
