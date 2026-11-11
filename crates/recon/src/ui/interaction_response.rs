use std::path::Path;

use schemars::JsonSchema;
use serde::Serialize;

use recon_core::error::{ActionMechanism, ReleaseState};
use recon_input_macos::request::{ClickMethod, MouseButton, ScrollUnit};
use recon_window_server_macos::window::WindowId;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionApplication<'a> {
    pub path: &'a Path,
    pub pid: u32,
    pub pidversion: u32,
}

#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ActionOperation {
    Click,
    Scroll,
    Key,
}

#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Unverified,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionElement {
    pub role: Option<String>,
    pub title: Option<String>,
    pub identifier: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClickMethodResult {
    pub requested: ClickMethod,
    #[schemars(schema_with = "effective_click_method_schema")]
    pub effective: ClickMethod,
    pub reason: &'static str,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClickResponse<'a> {
    pub application: ActionApplication<'a>,
    pub window: WindowId,
    pub operation: ActionOperation,
    #[schemars(schema_with = "action_mechanism_schema")]
    pub mechanism: ActionMechanism,
    pub outcome: Outcome,
    pub method: ClickMethodResult,
    pub button: MouseButton,
    pub count: u8,
    pub modifiers: Vec<&'static str>,
    pub location: LocalPoint,
    pub element: Option<ActionElement>,
    pub target_key_window_changed: bool,
    pub release: ReleaseState,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScrollDelta {
    pub x: i32,
    pub y: i32,
    pub unit: ScrollUnit,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScrollResponse<'a> {
    pub application: ActionApplication<'a>,
    pub window: WindowId,
    pub operation: ActionOperation,
    #[schemars(schema_with = "action_mechanism_schema")]
    pub mechanism: ActionMechanism,
    pub outcome: Outcome,
    pub delta: ScrollDelta,
    pub target_key_window_changed: bool,
    pub release: ReleaseState,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ChordResult {
    pub modifiers: Vec<&'static str>,
    pub key: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum KeyWindowMechanism {
    AlreadyKey,
    WindowServerRecords,
    UikitHelper,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KeyWindowResult {
    pub mechanism: KeyWindowMechanism,
    pub before: Option<WindowId>,
    pub after: Option<WindowId>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct KeyResponse<'a> {
    pub application: ActionApplication<'a>,
    pub window: WindowId,
    pub operation: ActionOperation,
    #[schemars(schema_with = "action_mechanism_schema")]
    pub mechanism: ActionMechanism,
    pub outcome: Outcome,
    pub chord: ChordResult,
    pub key_window: KeyWindowResult,
    pub target_key_window_changed: bool,
    pub release: ReleaseState,
}

fn action_mechanism_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    serde_json::from_value(serde_json::json!({
        "type": "string",
        "enum": ["ax-press", "mouse-events", "scroll-wheel", "key-events"]
    }))
    .expect("action mechanism schema")
}

fn effective_click_method_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    serde_json::from_value(serde_json::json!({"type":"string","enum":["ax","mouse"]}))
        .expect("effective click method schema")
}
