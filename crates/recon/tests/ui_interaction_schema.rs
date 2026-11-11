use serde_json::{Value, json};

fn schema(arguments: &[&str]) -> Value {
    let output =
        std::process::Command::new(env!("CARGO_BIN_EXE_recon")).args(arguments).output().unwrap();
    assert!(output.status.success(), "{arguments:?}: {output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn validate(command: &[&str], instance: Value) {
    let specification =
        schema(&std::iter::once("schema").chain(command.iter().copied()).collect::<Vec<_>>());
    let validator = jsonschema::draft202012::new(&specification["command"]["output"]).unwrap();
    let errors =
        validator.iter_errors(&instance).map(|error| error.to_string()).collect::<Vec<_>>();
    assert!(errors.is_empty(), "{command:?}: {errors:?}; {instance}");
}

fn application() -> Value {
    json!({"path": "/Applications/Notes.app", "pid": 42, "pidversion": 7})
}

#[test]
fn proposal0007_interaction_commands_publish_effects_and_action_errors() {
    let index = schema(&["schema"]);
    let paths = index["commands"].as_array().unwrap();
    for command in ["click", "scroll", "key"] {
        let path = json!(["ui", command]);
        assert!(paths.iter().any(|candidate| candidate["path"] == path), "{paths:?}");
        let specification = schema(&["schema", "ui", command]);
        let effects = specification["command"]["effects"].as_array().unwrap();
        assert!(effects.contains(&json!("sends-input")), "{effects:?}");
        assert!(effects.contains(&json!("changes-target-key-window")), "{effects:?}");
        let errors = specification["command"]["errors"].as_array().unwrap();
        for code in [
            "ACTION_UNAVAILABLE",
            "AMBIGUOUS_ELEMENT",
            "STALE_TARGET",
            "ACTION_FAILED",
            "ACTION_TIMEOUT",
            "UI_INTERFERENCE",
            "TIMEOUT",
        ] {
            assert!(errors.contains(&json!(code)), "{command}: {errors:?}");
        }
    }
    let key = schema(&["schema", "ui", "key"]);
    assert!(key["command"]["effects"].as_array().unwrap().contains(&json!("executes-target-code")));
    assert!(key["command"]["errors"].as_array().unwrap().contains(&json!("INSPECTION_BUSY")));
}

#[test]
fn proposal0007_click_result_schema_covers_delivery_and_target_local_change() {
    validate(
        &["ui", "click"],
        json!({
            "application": application(),
            "window": 123,
            "operation": "click",
            "mechanism": "ax-press",
            "outcome": "unverified",
            "method": {"requested": "auto", "effective": "ax", "reason": "ax-press-advertised"},
            "button": "left",
            "count": 1,
            "modifiers": [],
            "location": {"x": 160.0, "y": 84.0},
            "element": {"role": "AXButton", "title": "Share", "identifier": null},
            "targetKeyWindowChanged": false,
            "release": "not-needed"
        }),
    );
}

#[test]
fn proposal0007_scroll_result_schema_preserves_both_signed_axes_and_units() {
    validate(
        &["ui", "scroll"],
        json!({
            "application": application(),
            "window": 123,
            "operation": "scroll",
            "mechanism": "scroll-wheel",
            "outcome": "unverified",
            "delta": {"x": -5, "y": 300, "unit": "pixel"},
            "targetKeyWindowChanged": false,
            "release": "not-needed"
        }),
    );
}

#[test]
fn proposal0007_key_result_schema_reports_key_window_provenance_and_release() {
    validate(
        &["ui", "key"],
        json!({
            "application": application(),
            "window": 123,
            "operation": "key",
            "mechanism": "key-events",
            "outcome": "unverified",
            "chord": {"modifiers": ["cmd"], "key": "b"},
            "keyWindow": {"mechanism": "uikit-helper", "before": 122, "after": 123},
            "targetKeyWindowChanged": true,
            "release": "complete"
        }),
    );
}
