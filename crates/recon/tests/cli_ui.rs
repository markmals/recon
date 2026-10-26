//! Command-line behavior of `recon ui` that never reaches a privacy permission check. Argument
//! mistakes are rejected before Recon observes anything, and a target that cannot be resolved
//! fails after the responsibility re-execution but before any permission is checked, so these
//! tests are safe to run against the ad-hoc `cargo` build.

use std::process::{Command, Output};

use recon::request::USAGE;

fn recon(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_recon")).args(arguments).output().expect("start recon")
}

fn failure(arguments: &[&str]) -> String {
    let output = recon(arguments);
    assert!(!output.status.success(), "recon {arguments:?} should fail");
    assert!(output.stdout.is_empty(), "recon {arguments:?} should print no result");
    let diagnostic = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    assert!(!diagnostic.contains("panicked at"), "{diagnostic}");
    diagnostic
}

fn rejects(arguments: &[&str]) {
    assert_eq!(failure(arguments), format!("recon: {USAGE}\n"), "recon {arguments:?}");
}

#[test]
fn a_ui_command_without_a_target_is_a_usage_error() {
    rejects(&["ui"]);
    rejects(&["ui", "windows"]);
    rejects(&["ui", "tree"]);
    rejects(&["ui", "screenshot"]);
}

#[test]
fn a_flag_a_ui_command_does_not_take_is_a_usage_error() {
    rejects(&["ui", "windows", "Notes", "--window", "17"]);
    rejects(&["ui", "tree", "Notes", "--out", "/tmp/tree.png"]);
    rejects(&["ui", "screenshot", "Notes"]);
    rejects(&["ui", "inspect", "Notes"]);
}

#[test]
fn the_usage_text_covers_both_command_groups() {
    let diagnostic = failure(&["app"]);
    assert!(diagnostic.contains("recon app list"), "{diagnostic}");
    assert!(diagnostic.contains("recon ui windows"), "{diagnostic}");
    assert!(diagnostic.contains("recon ui tree"), "{diagnostic}");
    assert!(diagnostic.contains("recon ui screenshot"), "{diagnostic}");
}

#[cfg(unix)]
#[test]
fn a_malformed_ui_argument_reports_a_cli_error_instead_of_panicking() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let output = Command::new(env!("CARGO_BIN_EXE_recon"))
        .args([OsString::from("ui"), OsString::from("tree"), OsString::from_vec(vec![0xff])])
        .output()
        .expect("start recon");

    assert!(!output.status.success());
    let diagnostic = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    assert_eq!(diagnostic, "recon: invalid UTF-8 argument\n");
}

#[cfg(target_os = "macos")]
#[test]
fn an_unresolvable_target_fails_before_any_permission_is_checked() {
    let diagnostic = failure(&["ui", "windows", "/recon-has-no-such.app"]);
    assert!(diagnostic.contains("/recon-has-no-such.app"), "{diagnostic}");
    assert!(!diagnostic.contains("permission"), "{diagnostic}");
}

#[cfg(target_os = "macos")]
#[test]
fn the_executable_identifies_itself_to_macos_privacy_settings_as_recon() {
    let output = Command::new("/bin/launchctl")
        .args(["plist", "__TEXT,__info_plist", env!("CARGO_BIN_EXE_recon")])
        .output()
        .expect("start launchctl");

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let embedded = String::from_utf8(output.stdout).expect("UTF-8 property list");
    assert!(embedded.contains(r#""CFBundleIdentifier" = "me.malstrom.recon";"#), "{embedded}");
    assert!(embedded.contains(r#""CFBundleName" = "recon";"#), "{embedded}");
}

#[cfg(not(target_os = "macos"))]
#[test]
fn ui_observation_reports_the_host_it_supports() {
    assert_eq!(
        failure(&["ui", "windows", "Notes"]),
        "recon: UI observation is supported only on macOS\n"
    );
}
