use std::process::Command;

use serde_json::{Value, json};

fn invoke(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_recon")).args(arguments).output().expect("run recon")
}

#[test]
fn proposal_0004_invalid_options_use_the_shared_compact_error_contract() {
    for (arguments, argument) in [
        (vec!["app", "list", "--running=yes"], "--running"),
        (vec!["app", "list", "--running", "--running"], "--running"),
        (vec!["ui", "tree", "Notes", "--depth", "65"], "--depth"),
        (vec!["ui", "tree", "Notes", "--depth", "１"], "--depth"),
        (vec!["ui", "tree", "Notes", "--depth"], "--depth"),
        (vec!["ui", "tree", "Notes", "--no-menu-bar", "--window", "0"], "--no-menu-bar"),
        (vec!["ui", "windows", "Notes", "--no-menu-bar"], "--no-menu-bar"),
    ] {
        let output = invoke(&arguments);
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        let text = String::from_utf8(output.stderr).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        let report: Value = serde_json::from_str(&text).expect("one JSON error");
        assert!(text.ends_with('\n'));
        assert_eq!(report["error"]["code"], "INVALID_ARGUMENT");
        assert_eq!(report["error"]["retryable"], false);
        assert_eq!(report["error"]["details"]["argument"], argument);
        if arguments.contains(&"65") {
            assert!(report["error"]["message"].as_str().unwrap().contains("64"));
        }
    }
}

#[test]
fn proposal_0004_menu_window_conflict_names_both_incompatible_options_in_either_order() {
    for options in [["--no-menu-bar", "--window", "0"], ["--window", "0", "--no-menu-bar"]] {
        let mut arguments = vec!["ui", "tree", "Notes"];
        arguments.extend(options);
        let output = invoke(&arguments);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        let report: Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(report["error"]["code"], "INVALID_ARGUMENT");
        let message = report["error"]["message"].as_str().unwrap();
        assert!(message.contains("--no-menu-bar") && message.contains("--window"), "{message}");
    }
}

#[test]
fn proposal_0004_schema_advertises_only_the_registered_narrowing_options() {
    for (path, options) in [
        (vec!["schema", "app", "list"], vec![("--running", "boolean")]),
        (vec!["schema", "ui", "tree"], vec![("--depth", "integer"), ("--no-menu-bar", "boolean")]),
    ] {
        let output = invoke(&path);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        let specification: Value = serde_json::from_slice(&output.stdout).unwrap();
        let advertised = specification["command"]["options"].as_array().unwrap();
        for (name, kind) in options {
            let option =
                advertised.iter().find(|option| option["name"] == name).expect("registered option");
            assert_eq!(option["type"], kind);
            assert_eq!(option["repeatable"], false);
            assert_eq!(option["required"], false);
            if name == "--depth" {
                assert_eq!(option["constraints"]["minimum"], 0);
                assert_eq!(option["constraints"]["maximum"], 64);
                assert_eq!(option["constraints"]["pattern"], "^[0-9]+$");
                assert_eq!(option["default"], Value::Null);
            } else {
                assert_eq!(option["default"], false);
            }
        }
        assert!(!advertised.iter().any(|option| option["name"] == "--compact"));
    }
}

#[test]
fn proposal_0004_tree_schema_requires_the_complete_envelope_and_sole_sparse_node_shape() {
    let specification: Value =
        serde_json::from_slice(&invoke(&["schema", "ui", "tree"]).stdout).unwrap();
    let validator = jsonschema::draft202012::new(&specification["command"]["output"]).unwrap();
    let envelope = |root: Value| {
        json!({
            "application": {"path": "/Fixture.app", "pid": 42},
            "windowId": null,
            "truncated": false,
            "narrowed": false,
            "root": root,
        })
    };
    for node in [
        json!({}),
        json!({"role": "AXButton", "enabled": false, "focused": false, "title": "", "value": 0}),
        json!({"repeated": true}),
        json!({"role": "AXWindow", "repeated": true}),
        json!({"error": "element-unavailable"}),
        json!({"error": "timeout"}),
        json!({"error": "read-failed", "axError": -25200}),
        json!({"depthLimited": true}),
        json!({"menuBarExcluded": true}),
        json!({"children": [{}], "actions": ["AXPress"]}),
    ] {
        assert!(validator.is_valid(&envelope(node.clone())), "{node}");
    }
    for node in [
        json!({"role": null}),
        json!({"value": null}),
        json!({"focused": null}),
        json!({"frame": null}),
        json!({"actions": []}),
        json!({"children": []}),
        json!({"depthLimited": false}),
        json!({"menuBarExcluded": false}),
        json!({"childrenOmitted": false}),
        json!({"valueTruncated": false}),
        json!({"role": null, "repeated": true}),
        json!({"repeated": false}),
        json!({"error": "read-failed"}),
        json!({"error": "timeout", "children": [{}]}),
        json!({"unexpected": true}),
    ] {
        assert!(!validator.is_valid(&envelope(node.clone())), "{node}");
    }
    for field in ["application", "windowId", "truncated", "narrowed", "root"] {
        let mut missing = envelope(json!({}));
        missing.as_object_mut().unwrap().remove(field);
        assert!(!validator.is_valid(&missing), "missing {field}");
    }
}
