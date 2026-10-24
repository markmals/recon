//! Running one `recon app` command against the applications on this Mac.

use std::path::Path;

use serde::Serialize;

use super::workspace::launch;
use super::{applications, encode, with_target};
use crate::application::Application;
use crate::request::AppRequest;

/// How `app list` found its applications: the Spotlight index, plus whatever is running.
const DISCOVERY: &str = "indexed-and-running";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListedApplication<'a> {
    name: &'a str,
    bundle_id: Option<&'a str>,
    path: &'a Path,
    running_pids: &'a [u32],
}

#[derive(Serialize)]
struct ListResponse<'a> {
    discovery: &'static str,
    applications: Vec<ListedApplication<'a>>,
}

#[derive(Serialize)]
struct InspectResponse<'a> {
    application: &'a Application,
}

#[derive(Serialize)]
struct LaunchResponse<'a> {
    path: &'a Path,
    pid: u32,
}

/// Runs one `recon app` command and returns its result as JSON.
pub fn investigate(request: &AppRequest) -> Result<String, String> {
    match request {
        AppRequest::List => list(),
        AppRequest::Inspect { target } => {
            with_target(target, |application| encode(&InspectResponse { application }))
        }
        AppRequest::Launch { target } => with_target(target, launch_application),
    }
}

fn list() -> Result<String, String> {
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

fn launch_application(application: &Application) -> Result<String, String> {
    if application.executable_path.is_none() {
        return Err(format!(
            "cannot launch {}: application executable is missing",
            application.path.display()
        ));
    }
    let pid = launch(&application.path)?;
    encode(&LaunchResponse { path: &application.path, pid })
}
