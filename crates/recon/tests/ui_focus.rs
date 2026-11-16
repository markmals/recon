#![cfg(target_os = "macos")]

//! Watching the Mac from outside Recon while a `ui` command runs.
//!
//! The watch itself lives in `ui_observation/desktop.rs`, because the interaction evidence
//! holds its operations to the same continuous reading. These checks are the reading ones: a
//! command that only observes must leave every dimension of the contract where it found it.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

#[path = "ui_observation/harness.rs"]
#[allow(dead_code)]
mod harness;

#[path = "ui_observation/desktop.rs"]
#[allow(dead_code)]
mod desktop;

use desktop::Watch;
use harness::Fixture;

#[test]
fn watching_this_mac_covers_every_dimension_the_contract_names() {
    let watch = Watch::start();

    thread::sleep(Duration::from_millis(200));
    let seen = watch.stop();

    assert!(
        seen.unreadable.is_empty(),
        "this Mac does not report {:?}; Recon's guard fails closed there instead of reporting a \
         quiet desktop",
        seen.unreadable
    );
    // Two looks are a before-and-after pair. Anything the contract cares about can happen and be
    // undone between them, which is why the watch has to be continuous.
    assert!(seen.samples > 2, "{seen:?}");
    assert!(seen.longest_interval < Duration::from_millis(500), "{seen:?}");
}

#[test]
#[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted, and the fixture bundle; run vp run test:focus"]
fn reading_the_windows_of_a_background_application_disturbs_nothing() {
    let fixture = Fixture::launch();
    let watch = Watch::start();

    let output = recon(&["ui", "windows", fixture.target()]);

    let seen = watch.stop();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(seen.samples > 2, "the watch has to span the command, not bracket it: {seen:?}");
    assert!(seen.undisturbed(), "recon ui windows disturbed the desktop: {seen:?}");
}

#[test]
#[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted, and the fixture bundle; run vp run test:focus"]
fn reading_the_accessibility_tree_of_a_background_application_disturbs_nothing() {
    let fixture = Fixture::launch();
    let watch = Watch::start();

    let output = recon(&["ui", "ax", "tree", fixture.target()]);

    let seen = watch.stop();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(seen.samples > 2, "{seen:?}");
    assert!(seen.undisturbed(), "recon ui ax tree disturbed the desktop: {seen:?}");
}

/// The counter-case: a command that is meant to activate an application must be visible to the
/// same watch. A watch that reports a quiet desktop here would report one for anything.
#[test]
#[ignore = "activates an application on the desktop; needs target/signed/recon and the fixture bundle"]
fn an_activating_command_is_visible_to_the_same_watch() {
    let fixture = Fixture::launch();
    let watch = Watch::start();

    let output = recon(&["app", "launch", fixture.target()]);

    let seen = watch.stop();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(
        seen.foreground.contains(&fixture.pid),
        "app launch activates the application, and the watch must see it: {seen:?}"
    );
}

fn recon(arguments: &[&str]) -> Output {
    Command::new(signed_recon()).args(arguments).output().expect("start recon")
}

fn signed_recon() -> PathBuf {
    if let Some(recon) = std::env::var_os("RECON_UI_BINARY") {
        return PathBuf::from(recon);
    }
    let recon = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/signed/recon");
    assert!(recon.is_file(), "{} is missing; build it with `vp run recon:sign`", recon.display());
    recon
}
