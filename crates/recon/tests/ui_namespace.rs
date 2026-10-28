//! Proposal 0005: the `ui` namespace is nested. Accessibility queries live under `ui ax`, so the
//! accessibility tree is `recon ui ax tree`. The old flat `ui tree` spelling and a top-level `ax`
//! command are rejected rather than aliased, and the nested groups are discoverable through the
//! same registry, help, and schema the flat commands used.

use serde_json::{Value, json};

fn recon(arguments: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_recon")).args(arguments).output().unwrap()
}

fn success(arguments: &[&str]) -> Vec<u8> {
    let output = recon(arguments);
    assert_eq!(output.status.code(), Some(0), "{arguments:?}: {output:?}");
    assert!(output.stderr.is_empty(), "{arguments:?}: {output:?}");
    output.stdout
}

fn usage_error(arguments: &[&str]) -> Value {
    let output = recon(arguments);
    assert_eq!(output.status.code(), Some(2), "{arguments:?}: {output:?}");
    assert!(output.stdout.is_empty(), "{arguments:?}: {output:?}");
    let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(envelope["error"]["code"], "INVALID_ARGUMENT", "{envelope}");
    envelope["error"].clone()
}

#[test]
fn the_accessibility_tree_is_a_nested_ui_ax_command() {
    let help = String::from_utf8(success(&["ui", "ax", "tree", "--help"])).unwrap();
    assert!(help.contains("accessibility tree"), "{help}");

    let index: Value = serde_json::from_slice(&success(&["schema"])).unwrap();
    let paths: Vec<Value> = index["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["path"].clone())
        .collect();
    assert!(paths.contains(&json!(["ui", "ax", "tree"])), "{paths:?}");
    assert!(!paths.contains(&json!(["ui", "tree"])), "{paths:?}");

    let spec: Value = serde_json::from_slice(&success(&["schema", "ui", "ax", "tree"])).unwrap();
    assert_eq!(spec["command"]["path"], json!(["ui", "ax", "tree"]));
    let options: Vec<&str> = spec["command"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| option["name"].as_str().unwrap())
        .collect();
    assert!(options.contains(&"--depth"), "{options:?}");
    assert!(options.contains(&"--no-menu-bar"), "{options:?}");
}

#[test]
fn the_old_flat_ui_tree_spelling_is_rejected() {
    let error = usage_error(&["ui", "tree", "Calculator"]);
    assert_eq!(error["details"]["value"], "tree");
    let accepted = error["details"]["accepted"].as_array().unwrap();
    assert!(accepted.contains(&json!("ax")), "{accepted:?}");
    assert!(accepted.contains(&json!("windows")), "{accepted:?}");
    assert!(accepted.contains(&json!("screenshot")), "{accepted:?}");
    assert!(!accepted.contains(&json!("tree")), "{accepted:?}");
}

#[test]
fn there_is_no_top_level_ax_command() {
    let error = usage_error(&["ax", "tree", "Calculator"]);
    assert_eq!(error["details"]["value"], "ax");
    let accepted = error["details"]["accepted"].as_array().unwrap();
    assert!(accepted.contains(&json!("ui")), "{accepted:?}");
    assert!(!accepted.contains(&json!("ax")), "{accepted:?}");
}

#[test]
fn the_ui_ax_group_lists_only_its_operations() {
    let error = usage_error(&["ui", "ax"]);
    assert_eq!(error["details"]["accepted"], json!(["tree"]));

    let help = String::from_utf8(success(&["ui", "ax", "--help"])).unwrap();
    assert!(help.contains("tree"), "{help}");
}

#[test]
fn the_ui_group_exposes_the_nested_ax_subgroup_beside_its_operations() {
    let error = usage_error(&["ui"]);
    let accepted = error["details"]["accepted"].as_array().unwrap();
    assert!(accepted.contains(&json!("ax")), "{accepted:?}");
    assert!(accepted.contains(&json!("windows")), "{accepted:?}");
    assert!(accepted.contains(&json!("screenshot")), "{accepted:?}");
}
