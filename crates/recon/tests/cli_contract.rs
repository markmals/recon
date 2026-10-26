use std::process::{Command, Output};

use proptest::prelude::*;
use serde_json::{Value, json};

fn recon(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_recon")).args(arguments).output().unwrap()
}

fn success(arguments: &[&str]) -> Vec<u8> {
    let output = recon(arguments);
    assert_eq!(output.status.code(), Some(0), "{arguments:?}: {:?}", output);
    assert!(output.stderr.is_empty(), "{arguments:?}: {:?}", output);
    output.stdout
}

fn invalid(arguments: &[&str]) -> Value {
    let output = recon(arguments);
    assert_eq!(output.status.code(), Some(2), "{arguments:?}: {:?}", output);
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert_eq!(output.stderr.last(), Some(&b'\n'));
    let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(envelope["error"]["code"], "INVALID_ARGUMENT");
    assert_eq!(envelope["error"]["retryable"], false);
    assert_eq!(envelope.as_object().unwrap().len(), 1);
    envelope["error"].clone()
}

#[test]
fn proposal0003_orientation_is_successful_and_help_overrides_bad_arguments() {
    let top = success(&[]);
    for arguments in [vec!["--help"], vec!["-h"], vec!["help"]] {
        assert_eq!(success(&arguments), top);
    }
    let top = String::from_utf8(top).unwrap();
    for path in [
        "app list",
        "app inspect",
        "app launch",
        "ui windows",
        "ui tree",
        "ui screenshot",
        "schema",
        "context",
    ] {
        assert!(top.contains(path), "missing {path}: {top}");
    }
    assert!(!top.contains('\u{1b}'));
    assert!(top.contains("recon schema <command>"));
    let tree = success(&["ui", "tree", "--help"]);
    for arguments in [
        vec!["help", "ui", "tree"],
        vec!["ui", "tree", "--window", "abc", "--help"],
        vec!["ui", "tree", "--pid", "--help"],
    ] {
        assert_eq!(success(&arguments), tree);
    }
    assert_eq!(success(&["app", "--help"]), success(&["help", "app"]));
    assert_eq!(success(&["context", "-h"]), success(&["help", "context"]));
    let version = String::from_utf8(success(&["--version"])).unwrap();
    assert!(version.starts_with(concat!("recon ", env!("CARGO_PKG_VERSION"))));
    assert_eq!(version.lines().count(), 1);
    invalid(&["-v"]);
    invalid(&["-V"]);
}

#[test]
fn proposal0003_usage_errors_carry_recovery_data() {
    let error = invalid(&["ui", "tree", "Calculator", "--window", "abc"]);
    assert_eq!(error["command"], json!(["ui", "tree"]));
    assert_eq!(
        error["details"],
        json!({"argument":"--window","value":"abc","expected":"a window id written as decimal digits","accepted":null})
    );
    let error = invalid(&["ui", "screenshot", "Calculator"]);
    assert_eq!(error["details"]["argument"], "--out");
    assert!(error["details"]["value"].is_null());
    let error = invalid(&["ui", "windows", "Calculator", "--bogus"]);
    assert_eq!(error["details"]["accepted"], json!(["--pid"]));
    assert_eq!(invalid(&["app", "list", "--json"])["details"]["accepted"], json!([]));
    assert_eq!(invalid(&["app"])["details"]["accepted"], json!(["list", "inspect", "launch"]));
    assert_eq!(invalid(&["app", "inspect", "Safari", "Mail"])["details"]["value"], "Mail");
    let error = invalid(&["ui", "tree", "Calculator", "--pid", "1", "--pid", "2"]);
    assert_eq!(error["details"]["argument"], "--pid");
    assert_eq!(error["details"]["value"], "2");
    for arguments in [
        vec!["ui", "tree", "Notes", "--pid", "0"],
        vec!["ui", "tree", "Notes", "--pid", "--window", "9"],
        vec!["ui", "tree", "Notes", "--window"],
        vec!["context", "extra"],
        vec!["schema", "ui", "absent"],
    ] {
        invalid(&arguments);
    }
}

#[test]
fn proposal0003_registry_is_discoverable_and_constraints_are_enforced() {
    let index: Value = serde_json::from_slice(&success(&["schema"])).unwrap();
    assert_eq!(index["schemaVersion"], 1);
    let paths = index["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["path"].clone())
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        json!([
            ["app", "list"],
            ["app", "inspect"],
            ["app", "launch"],
            ["ui", "windows"],
            ["ui", "tree"],
            ["ui", "screenshot"],
            ["schema"],
            ["context"]
        ])
        .as_array()
        .unwrap()
        .clone()
    );
    let mut covered = std::collections::BTreeSet::new();
    let codes = index["errors"]["codes"].as_array().unwrap();
    for command in index["commands"].as_array().unwrap() {
        let mut arguments = vec!["schema"];
        arguments
            .extend(command["path"].as_array().unwrap().iter().map(|word| word.as_str().unwrap()));
        let specification: Value = serde_json::from_slice(&success(&arguments)).unwrap();
        assert_eq!(specification["command"]["path"], command["path"]);
        assert_eq!(
            specification["command"]["output"]["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        for code in specification["command"]["errors"].as_array().unwrap() {
            assert!(codes.iter().any(|known| known["code"] == *code));
            covered.insert(code.as_str().unwrap().to_owned());
        }
        let mut help = arguments[1..].to_vec();
        help.push("--help");
        success(&help);
    }
    assert_eq!(covered.len(), codes.len());
    let tree: Value = serde_json::from_slice(&success(&["schema", "ui", "tree"])).unwrap();
    let pid = tree["command"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| option["name"] == "--pid")
        .unwrap();
    assert_eq!(pid["constraints"]["pattern"], "^[0-9]+$");
    assert_eq!(pid["constraints"]["minimum"], 1);
    assert_eq!(pid["repeatable"], false);
    assert_eq!(tree["command"]["effects"], json!([]));
}

proptest! {
    #![proptest_config(ProptestConfig { cases:32, failure_persistence:None, ..ProptestConfig::default() })]
    #[test]
    fn proposal0003_non_decimal_pid_strings_never_reach_the_host(value in "[^\\x00]{0,24}".prop_filter("not a positive plain u32", |text| text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) || text.parse::<u32>().ok().is_none_or(|number| number == 0))) {
        invalid(&["ui", "tree", "Notes", "--pid", &value]);
    }
}

#[cfg(unix)]
#[test]
fn proposal0003_non_utf8_arguments_have_a_structured_usage_error() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let output = Command::new(env!("CARGO_BIN_EXE_recon"))
        .args([OsString::from("app"), OsString::from("inspect"), OsString::from_vec(vec![0xff])])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(envelope["error"]["code"], "INVALID_ARGUMENT");
    assert_eq!(envelope["error"]["details"]["expected"], "a UTF-8 argument");
}

#[test]
fn proposal0003_separator_without_a_command_is_an_actionable_usage_error() {
    let error = invalid(&["--"]);
    assert!(error["command"].is_null());
    assert_eq!(error["details"]["accepted"], json!(["app", "ui", "schema", "context"]));
}

#[test]
fn proposal0003_a_missing_operation_has_no_offending_value_or_resolved_command() {
    let error = invalid(&["app"]);
    assert!(error["command"].is_null());
    assert!(error["details"]["value"].is_null());
    assert_eq!(error["details"]["accepted"], json!(["list", "inspect", "launch"]));
}

#[cfg(unix)]
#[test]
fn proposal0003_help_precedes_even_non_utf8_argument_validation() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let output = Command::new(env!("CARGO_BIN_EXE_recon"))
        .args([
            OsString::from("ui"),
            OsString::from("tree"),
            OsString::from_vec(vec![0xff]),
            OsString::from("--help"),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, success(&["ui", "tree", "--help"]));
}
