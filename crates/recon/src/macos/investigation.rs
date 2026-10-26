//! Running one `recon app` command against the applications on this Mac.

use serde_json::json;

use super::workspace::launch;
use super::{applications, encode, with_target};
use crate::application::Application;
use crate::application::response::{
    InspectResponse, LaunchResponse, ListResponse, ListedApplication,
};
use crate::error::{ErrorCode, Failure};
use crate::request::AppRequest;

/// How `app list` found its applications: the Spotlight index, plus whatever is running.
const DISCOVERY: &str = "indexed-and-running";

/// Runs one `recon app` command and returns its result as JSON.
pub fn investigate(request: &AppRequest) -> Result<String, Failure> {
    match request {
        AppRequest::List => list(),
        AppRequest::Inspect { target } => {
            with_target(target, |application| encode(&InspectResponse { application }))
        }
        AppRequest::Launch { target } => with_target(target, launch_application),
    }
}

fn list() -> Result<String, Failure> {
    let applications = applications()?;
    let entries = applications
        .iter()
        .map(|application| ListedApplication {
            name: &application.name,
            bundle_id: application.bundle_id.as_deref(),
            path: &application.path,
            running_pids: &application.running_pids,
        })
        .collect();
    encode(&ListResponse { discovery: DISCOVERY, applications: entries })
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
