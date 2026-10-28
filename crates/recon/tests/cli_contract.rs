use std::process::{Command, Output};

use proptest::prelude::*;
use serde_json::{Value, json};
mod support;

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
    if matches!(arguments.first(), Some(&"ui" | &"app")) {
        return invalid_request(arguments);
    }
    usage_error(recon(arguments))
}

fn usage_error(output: Output) -> Value {
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert_eq!(output.stderr.last(), Some(&b'\n'));
    let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
    support::validate_failure(&envelope, &[]);
    assert_eq!(envelope["error"]["code"], "INVALID_ARGUMENT");
    assert_eq!(envelope["error"]["retryable"], false);
    assert_eq!(envelope.as_object().unwrap().len(), 1);
    envelope["error"].clone()
}

fn invalid_request(arguments: &[&str]) -> Value {
    let arguments = arguments.iter().map(|word| (*word).to_owned()).collect::<Vec<_>>();
    let failure = recon::request::parse_arguments(&arguments).unwrap_err();
    assert_eq!(failure.code, recon::error::ErrorCode::InvalidArgument);
    let envelope = serde_json::to_value(failure.report()).unwrap();
    support::validate_failure(&envelope, &[]);
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
        "ui ax tree",
        "ui screenshot",
        "schema",
        "context",
    ] {
        assert!(top.contains(path), "missing {path}: {top}");
    }
    assert!(!top.contains('\u{1b}'));
    assert!(top.contains("recon schema <command>"));
    let tree = success(&["ui", "ax", "tree", "--help"]);
    for arguments in [
        vec!["help", "ui", "ax", "tree"],
        vec!["ui", "ax", "tree", "--window", "abc", "--help"],
        vec!["ui", "ax", "tree", "--pid", "--help"],
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
    let error = invalid(&["ui", "ax", "tree", "Calculator", "--window", "abc"]);
    assert_eq!(error["command"], json!(["ui", "ax", "tree"]));
    assert_eq!(error["details"]["argument"], "--window");
    assert_eq!(error["details"]["value"], "abc");
    assert!(error["details"]["accepted"].is_null());
    let error = invalid(&["ui", "screenshot", "Calculator"]);
    assert_eq!(error["details"]["argument"], "--out");
    assert!(error["details"]["value"].is_null());
    let error = invalid(&["ui", "windows", "Calculator", "--bogus"]);
    assert_eq!(error["details"]["accepted"], json!(["--pid"]));
    assert_eq!(invalid(&["app", "list", "--json"])["details"]["accepted"], json!(["--running"]));
    assert_eq!(invalid(&["app"])["details"]["accepted"], json!(["list", "inspect", "launch"]));
    assert_eq!(invalid(&["app", "inspect", "Safari", "Mail"])["details"]["value"], "Mail");
    let error = invalid(&["ui", "ax", "tree", "Calculator", "--pid", "1", "--pid", "2"]);
    assert_eq!(error["details"]["argument"], "--pid");
    assert_eq!(error["details"]["value"], "2");
    for arguments in [
        vec!["ui", "ax", "tree", "Notes", "--pid", "0"],
        vec!["ui", "ax", "tree", "Notes", "--pid", "--window", "9"],
        vec!["ui", "ax", "tree", "Notes", "--window"],
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
            ["ui", "ax", "tree"],
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
    let tree: Value = serde_json::from_slice(&success(&["schema", "ui", "ax", "tree"])).unwrap();
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
    fn proposal0003_non_decimal_pid_strings_never_reach_the_host(value in "[^\\x00]{0,24}".prop_filter("not a positive plain u32 or help request", |text| !matches!(text.as_str(), "-h" | "--help") && (text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) || text.parse::<u32>().ok().is_none_or(|number| number == 0)))) {
        invalid(&["ui", "ax", "tree", "Notes", "--pid", &value]);
    }
    #[test]
    fn proposal0003_hyphen_prefixed_output_values_are_usage_errors(tail in "[a-z0-9]{0,24}") {
        prop_assume!(tail != "h");
        let value = format!("-{tail}");
        for arguments in [
            vec!["ui", "screenshot", "Notes", "--out", value.as_str()],
            vec!["ui", "screenshot", "Notes", &format!("--out={value}")],
        ] {
            let error = invalid_request(&arguments);
            prop_assert_eq!(&error["details"]["argument"], "--out");
        }
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
    support::validate_failure(&envelope, &[]);
    assert_eq!(envelope["error"]["code"], "INVALID_ARGUMENT");
    assert_eq!(envelope["error"]["command"], json!(["app", "inspect"]));
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
            OsString::from("ax"),
            OsString::from("tree"),
            OsString::from_vec(vec![0xff]),
            OsString::from("--help"),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, success(&["ui", "ax", "tree", "--help"]));
}

#[test]
fn proposal0003_help_word_inside_an_invalid_path_is_the_offending_operation() {
    let error = invalid(&["app", "help", "list"]);
    assert!(error["command"].is_null());
    assert_eq!(error["details"]["value"], "help");
    assert_eq!(error["details"]["accepted"], json!(["list", "inspect", "launch"]));
}

#[test]
fn proposal0003_unknown_options_are_not_matched_by_value_name_text() {
    let error = invalid(&["ui", "ax", "tree", "Calculator", "--PID", "1"]);
    assert_eq!(error["details"]["argument"], "--PID");
    assert_eq!(
        error["details"]["accepted"],
        json!(["--pid", "--window", "--depth", "--no-menu-bar"])
    );
}

#[test]
fn proposal0003_option_shaped_extra_positionals_keep_their_actual_role() {
    for (arguments, value) in [
        (vec!["ui", "ax", "tree", "Calculator", "--", "--window"], "--window"),
        (vec!["ui", "ax", "tree", "--window", "1", "Calculator", "--", "--window"], "--window"),
        (vec!["ui", "ax", "tree", "--window=9", "Notes", "--", "--window=9"], "--window=9"),
        (vec!["ui", "ax", "tree", "--window=9", "Notes", "--", "--window"], "--window"),
    ] {
        let error = invalid(&arguments);
        assert!(error["details"]["argument"].is_null());
        assert_eq!(error["details"]["value"], value);
        assert!(error["details"]["accepted"].is_null());
    }
}

#[test]
fn proposal0003_commands_without_positionals_do_not_advertise_a_target_recovery() {
    for arguments in [vec!["context", "extra"], vec!["app", "list", "surplus"]] {
        let error = invalid(&arguments);
        assert!(error["details"]["argument"].is_null());
        assert_eq!(error["details"]["accepted"], json!([]));
    }
}

#[test]
fn proposal0003_missing_option_values_differ_from_explicit_empty_values() {
    let error = invalid_request(&["ui", "screenshot", "Calculator", "--out"]);
    assert_eq!(error["details"]["argument"], "--out");
    assert!(error["details"]["value"].is_null());
    let error = invalid_request(&["ui", "ax", "tree", "Calculator", "--pid", ""]);
    assert_eq!(error["details"]["value"], "");
}

#[test]
fn proposal0003_invalid_hyphen_values_identify_the_option_waiting_for_them() {
    let error = invalid_request(&["ui", "windows", "Notes", "--pid", "-1"]);
    assert_eq!(error["details"]["argument"], "--pid");
    assert_eq!(error["details"]["value"], "-1");
    let error = invalid_request(&["ui", "screenshot", "Notes", "--out=--pid"]);
    assert_eq!(error["details"]["argument"], "--out");
    assert_eq!(error["details"]["value"], "--pid");
}

#[test]
fn proposal0003_output_value_rejection_is_a_real_cli_usage_envelope() {
    let scratch = tempfile::tempdir().unwrap();
    let missing = scratch.path().join("Missing.app");
    let error = usage_error(recon(&["ui", "screenshot", missing.to_str().unwrap(), "--out=--pid"]));
    assert_eq!(error["details"]["argument"], "--out");
    assert_eq!(error["details"]["value"], "--pid");
}

#[test]
fn proposal0003_output_path_constraints_describe_the_value_parser() {
    let specification: Value =
        serde_json::from_slice(&success(&["schema", "ui", "screenshot"])).unwrap();
    let option = specification["command"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| option["name"] == "--out")
        .unwrap();
    let validator = jsonschema::draft202012::new(&option["constraints"]).unwrap();
    for (path, accepted) in [
        ("shot.png", true),
        ("/tmp/shot.png", true),
        ("\nshot.png", true),
        ("-shot.png", false),
        ("--pid", false),
    ] {
        assert_eq!(validator.is_valid(&json!(path)), accepted, "{path:?}");
    }
}

#[test]
fn proposal0003_unknown_root_and_group_flags_have_flag_recovery_data() {
    for (arguments, offending, accepted) in [
        (vec!["-v"], "-v", vec!["--help", "-h", "--version"]),
        (vec!["-V"], "-V", vec!["--help", "-h", "--version"]),
        (vec!["-vv"], "-vv", vec!["--help", "-h", "--version"]),
        (vec!["app", "--bogus"], "--bogus", vec!["--help", "-h"]),
        (vec!["app", "-hh"], "-hh", vec!["--help", "-h"]),
    ] {
        let error = invalid(&arguments);
        assert!(error["command"].is_null());
        assert_eq!(error["details"]["argument"], offending);
        assert_eq!(error["details"]["accepted"], json!(accepted));
    }
}

#[test]
fn proposal0003_version_recovery_explains_that_it_takes_no_other_arguments() {
    let error = invalid(&["--version", "extra"]);
    assert_eq!(error["details"]["argument"], "--version");
    assert_eq!(error["details"]["accepted"], json!([]));
}

#[test]
fn proposal0003_unknown_attached_option_before_separator_remains_the_offending_option() {
    let error = invalid_request(&["ui", "ax", "tree", "Notes", "--bogus=1", "--", "--bogus"]);
    assert_eq!(error["details"]["argument"], "--bogus");
    assert!(error["details"]["value"].is_null());
}
