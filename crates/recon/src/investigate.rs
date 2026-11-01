//! Running one `recon app` command against the applications on this Mac.

use recon_applications_macos::application::Application;
use recon_applications_macos::discovery::{applications, with_target};
use recon_applications_macos::response::{InspectResponse, LaunchResponse, ListResponse};
use recon_applications_macos::workspace::launch;
use recon_core::error::{ErrorCode, Failure};
use serde_json::json;

use crate::output::encode;
use crate::request::AppRequest;

/// Runs one `recon app` command and returns its result as JSON.
///
/// # Errors
///
/// Whatever discovery, target resolution, or launching reports.
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
