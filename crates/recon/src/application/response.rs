use std::path::Path;

use schemars::JsonSchema;
use serde::Serialize;

use super::{Application, Pid};

/// How `app list` found its applications: the Spotlight index, plus whatever is running.
const INDEXED_AND_RUNNING: &str = "indexed-and-running";

/// How `app list --running` narrows that discovery: only the applications with a process.
const RUNNING: &str = "running";

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListedApplication<'a> {
    name: &'a str,
    bundle_id: Option<&'a str>,
    path: &'a Path,
    running_pids: &'a [Pid],
}

/// The result of `app list`. Its fields are private so that every listing comes from
/// [`ListResponse::new`], the one place where `--running` decides what a listing holds.
#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListResponse<'a> {
    discovery: &'static str,
    applications: Vec<ListedApplication<'a>>,
}

impl<'a> ListResponse<'a> {
    /// Lists the `discovered` applications, keeping only those with a running process when
    /// `running_only`, and naming in `discovery` which question the result answers.
    ///
    /// Discovery itself is untouched: both of its sources have already been consulted, and the
    /// entries kept here carry the fields, the order, and the process IDs discovery gave them.
    /// When nothing is running, `--running` leaves an empty list rather than an error.
    pub fn new(discovered: &'a [Application], running_only: bool) -> Self {
        let applications = discovered
            .iter()
            .filter(|application| !running_only || !application.running_pids.is_empty())
            .map(|application| ListedApplication {
                name: &application.name,
                bundle_id: application.bundle_id.as_deref(),
                path: &application.path,
                running_pids: &application.running_pids,
            })
            .collect();
        let discovery = if running_only { RUNNING } else { INDEXED_AND_RUNNING };
        Self { discovery, applications }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectResponse<'a> {
    pub application: &'a Application,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LaunchResponse<'a> {
    pub path: &'a Path,
    pub pid: Pid,
}
