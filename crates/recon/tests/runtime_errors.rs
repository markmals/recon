//! proposal.0003: a runtime failure of an `app` or `ui` command is one JSON error object on
//! standard error, carrying a stable code and the structured data needed to recover from it.
//!
//! Every case here is reached without a privacy permission and without opening an application, so
//! these tests are safe to run against the ad-hoc `cargo` build.
#![cfg(target_os = "macos")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;
mod support;

fn scratch() -> TempDir {
    tempfile::tempdir().expect("create the scratch directory")
}

/// Writes an application bundle whose `Info.plist` holds `body`.
fn write_bundle(path: &Path, body: &str) {
    let contents = path.join("Contents");
    fs::create_dir_all(&contents).expect("create the bundle");
    fs::write(contents.join("Info.plist"), body).expect("write the bundle's Info.plist");
}

fn application_plist(extra: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>CFBundleIdentifier</key><string>test.recon.bundle</string>{extra}</dict></plist>
"#
    )
}

fn canonical(path: &Path) -> String {
    path.canonicalize().expect("canonicalize the path").to_str().expect("a UTF-8 path").to_owned()
}

/// The `error` member of the one object a failed command printed to standard error.
fn failure(arguments: &[&str]) -> Value {
    let output =
        Command::new(env!("CARGO_BIN_EXE_recon")).args(arguments).output().expect("start recon");

    assert_eq!(output.status.code(), Some(1), "recon {arguments:?} should exit 1");
    assert!(output.stdout.is_empty(), "recon {arguments:?} should print no result");
    let diagnostic = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    assert!(!diagnostic.contains("panicked at"), "{diagnostic}");
    assert_eq!(
        diagnostic.lines().count(),
        1,
        "recon {arguments:?} should print one line, got {diagnostic}"
    );
    assert!(diagnostic.ends_with('\n'), "recon {arguments:?} should end its line: {diagnostic}");

    let reported: Value =
        serde_json::from_str(diagnostic.trim_end()).expect("one JSON error object");
    support::validate_failure(&reported, &[]);
    let error = reported["error"].clone();
    assert!(error.is_object(), "{reported}");
    assert!(error["message"].as_str().is_some_and(|message| !message.is_empty()), "{error}");
    error
}

fn failed_inspection(path: &Path) -> Value {
    failure(&["app", "inspect", path.to_str().expect("a UTF-8 path")])
}

#[test]
fn a_path_that_cannot_be_read_is_reported_as_a_missing_path() {
    let missing = PathBuf::from("/recon-has-no-such-directory/Missing.app");

    let error = failed_inspection(&missing);

    assert_eq!(error["code"], "NOT_FOUND");
    assert_eq!(error["retryable"], false);
    assert_eq!(error["details"]["kind"], "path");
    assert_eq!(error["details"]["path"], missing.to_str().unwrap());
    assert!(error["details"]["detail"].is_string(), "{error}");
}

#[test]
fn an_unreadable_info_plist_is_told_apart_from_a_directory_that_is_not_a_bundle() {
    let scratch = scratch();
    let unreadable = scratch.path().join("Unreadable.app");
    write_bundle(&unreadable, "this is not a property list");
    let listed = scratch.path().join("Listed.app");
    write_bundle(&listed, r#"<?xml version="1.0"?><plist version="1.0"><array/></plist>"#);

    for path in [&unreadable, &listed] {
        let error = failed_inspection(path);

        assert_eq!(error["code"], "NOT_AN_APPLICATION", "{path:?}");
        assert_eq!(error["retryable"], false, "{path:?}");
        assert_eq!(error["details"]["reason"], "unreadable-info-plist", "{path:?}");
        assert_eq!(error["details"]["path"], canonical(path), "{path:?}");
        assert!(error["details"]["detail"].is_string(), "{path:?}: {error}");
    }
}

#[test]
fn a_selector_that_matches_no_application_names_the_selector() {
    let error = failure(&["app", "inspect", "ReconHasNoSuchApplication"]);

    assert_eq!(error["code"], "NOT_FOUND");
    assert_eq!(error["details"]["kind"], "application");
    assert_eq!(error["details"]["selector"], "ReconHasNoSuchApplication");
    assert_eq!(error["details"]["path"], Value::Null);
}

#[test]
fn a_bundle_without_an_executable_is_never_handed_to_macos() {
    let scratch = scratch();
    let bundle = scratch.path().join("Broken App.app");
    write_bundle(
        &bundle,
        &application_plist("<key>CFBundleExecutable</key><string>absent</string>"),
    );

    let error = failure(&["app", "launch", bundle.to_str().expect("a UTF-8 path")]);

    assert_eq!(error["code"], "LAUNCH_FAILED");
    assert_eq!(error["retryable"], false);
    assert_eq!(error["details"]["reason"], "executable-missing");
    assert_eq!(error["details"]["application"], canonical(&bundle));
    assert_eq!(error["details"]["detail"], Value::Null);
}

#[test]
fn a_ui_command_against_a_bundle_that_is_not_running_says_so_before_any_permission() {
    let scratch = scratch();
    let bundle = scratch.path().join("Stopped.app");
    write_bundle(&bundle, &application_plist(""));

    let error = failure(&["ui", "windows", bundle.to_str().expect("a UTF-8 path")]);

    assert_eq!(error["code"], "NOT_RUNNING");
    assert_eq!(error["retryable"], false);
    assert_eq!(error["details"]["application"], canonical(&bundle));
}
