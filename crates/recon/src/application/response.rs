use std::path::Path;

use schemars::JsonSchema;
use serde::Serialize;

use super::{Application, Pid};

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ListedApplication<'a> {
    pub name: &'a str,
    pub bundle_id: Option<&'a str>,
    pub path: &'a Path,
    pub running_pids: &'a [Pid],
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListResponse<'a> {
    pub discovery: &'static str,
    pub applications: Vec<ListedApplication<'a>>,
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
