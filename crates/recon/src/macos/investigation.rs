//! Running one `recon app` command against the applications on this Mac.

use serde_json::json;

use super::workspace::launch;
use super::{applications, encode, with_target};
use crate::application::Application;
use crate::application::response::{InspectResponse, LaunchResponse, ListResponse};
use crate::error::{ErrorCode, Failure};
use crate::request::AppRequest;

/// Runs one `recon app` command and returns its result as JSON.
pub fn investigate(request: &AppRequest) -> Result<String, Failure> {
    match request {
        AppRequest::List { running } => list(*running),
        AppRequest::Inspect { target } => {
            with_target(target, |application| encode(&InspectResponse { application }))
        }
        AppRequest::Launch { target } => with_target(target, launch_application),
    }
}

fn list(running_only: bool) -> Result<String, Failure> {
    let applications = applications()?;
    encode(&ListResponse::new(&applications, running_only))
}

fn launch_application(application: &Application) -> Result<String, Failure> {
    if application.executable_path.is_none() {
        return Err(Failure::new(
            ErrorCode::LaunchFailed,
            format!(
                "cannot launch {}: application executable is missing",
                application.path.display()
            ),
            json!({
                "application": application.path.display().to_string(),
                "reason": "executable-missing",
                "detail": null,
            }),
        ));
    }
    let pid = launch(&application.path)?;
    encode(&LaunchResponse { path: &application.path, pid })
}
