#![cfg(target_os = "macos")]

//! Permission-gated observations of a live application, run against the locally signed
//! `target/signed/recon` and the fixture bundle. Every test is opt-in because macOS grants
//! Screen Recording and Accessibility to one executable at one path, which CI cannot have.

#[path = "ui_observation/harness.rs"]
mod harness;

#[path = "ui_observation/selective.rs"]
mod selective;

use harness::{
    Fixture, INSPECTOR_WINDOW_TITLE, MAIN_WINDOW_TITLE, TemporaryDirectory, backing_scale_factor,
    frontmost_pid, main_window_id, nodes, path_argument, png_pixel_size, run_recon, run_ui,
    window_titled,
};
use serde_json::Value;

/// The `error` member of the one JSON object a failed command printed to standard error.
fn reported_error(stderr: &[u8]) -> Value {
    let diagnostic = String::from_utf8_lossy(stderr);
    let reported: Value =
        serde_json::from_str(diagnostic.trim_end()).expect("one JSON error object");
    reported["error"].clone()
}

#[test]
#[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted; run vp run test:ui"]
fn windows_reports_both_fixture_windows_with_their_window_server_facts() {
    let fixture = Fixture::launch();

    let observation = run_ui(&["windows", fixture.target()]);

    assert_eq!(observation["application"]["path"], fixture.target());
    assert_eq!(observation["application"]["pid"].as_u64(), Some(u64::from(fixture.pid)));

    let main = window_titled(&observation, MAIN_WINDOW_TITLE);
    assert!(main["id"].as_u64().is_some_and(|id| id > 0), "{main}");
    assert_eq!(main["layer"].as_i64(), Some(0), "{main}");
    // A value, not `true`: a window is off screen whenever the active Space is another one,
    // which the fixture cannot control.
    assert!(main["onScreen"].as_bool().is_some(), "{main}");
    assert!(main["alpha"].as_f64().is_some(), "{main}");
    for edge in ["x", "y", "width", "height"] {
        assert!(main["bounds"][edge].as_f64().is_some(), "{main}");
    }

    let inspector = window_titled(&observation, INSPECTOR_WINDOW_TITLE);
    assert_ne!(inspector["id"], main["id"]);
}

#[test]
#[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted; run vp run test:ui"]
fn the_application_tree_exposes_the_fixture_menu_without_activating_the_fixture() {
    let fixture = Fixture::launch();

    let tree = run_ui(&["tree", fixture.target()]);

    assert_eq!(tree["windowId"], Value::Null);
    assert_eq!(tree["root"]["role"], "AXApplication");
    let fixture_menu = nodes(&tree["root"])
        .into_iter()
        .find(|node| node["role"] == "AXMenuBarItem" && node["title"] == "Fixture")
        .unwrap_or_else(|| panic!("no Fixture menu in {tree}"));
    let item_titles: Vec<&str> =
        nodes(fixture_menu).iter().filter_map(|node| node["title"].as_str()).collect();
    assert!(item_titles.contains(&"First Item"), "{fixture_menu}");
    assert!(item_titles.contains(&"Second Item"), "{fixture_menu}");

    assert_ne!(frontmost_pid(), Some(fixture.pid), "reading the tree activated the fixture");
}

#[test]
#[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted; run vp run test:ui"]
fn the_window_tree_exposes_the_controls_of_the_main_window() {
    let fixture = Fixture::launch();
    let main_window = main_window_id(&fixture);

    let tree = run_ui(&["tree", fixture.target(), "--window", &main_window.to_string()]);

    assert_eq!(tree["windowId"].as_u64(), Some(main_window));
    let elements = nodes(&tree["root"]);
    assert!(
        elements.iter().any(|node| node["role"] == "AXButton" && node["title"] == "Record"),
        "{tree}"
    );
    assert!(elements.iter().any(|node| node["value"] == "fixture value"), "{tree}");
    assert!(
        elements.iter().any(|node| node["role"] == "AXCheckBox" && node["title"] == "Enabled"),
        "{tree}"
    );
}

#[test]
#[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted; run vp run test:ui"]
fn a_window_without_an_accessibility_element_is_reported_unreachable() {
    let fixture = Fixture::launch();
    let observation = run_ui(&["windows", fixture.target()]);
    let untitled = observation["windows"]
        .as_array()
        .expect("a windows array")
        .iter()
        .find(|window| window["title"].is_null())
        .unwrap_or_else(|| panic!("AppKit gave the fixture no untitled window: {observation}"));
    let id = untitled["id"].as_u64().expect("a window id").to_string();

    let output = run_recon(&["ui", "tree", fixture.target(), "--window", &id]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = reported_error(&output.stderr);
    assert_eq!(error["code"], "NOT_FOUND");
    assert_eq!(error["details"]["kind"], "window-element");
    assert_eq!(error["details"]["windowId"].as_u64().map(|id| id.to_string()), Some(id));
}

#[test]
#[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted; run vp run test:ui"]
fn a_window_screenshot_is_written_at_the_backing_pixel_size_of_its_bounds() {
    let fixture = Fixture::launch();
    let main_window = main_window_id(&fixture);
    let directory = TemporaryDirectory::new();
    let out = directory.file("main.png");

    let capture = run_ui(&[
        "screenshot",
        fixture.target(),
        "--window",
        &main_window.to_string(),
        "--out",
        path_argument(&out),
    ]);

    assert_eq!(capture["method"], "skylight-hardware-capture");
    assert_eq!(capture["window"]["id"].as_u64(), Some(main_window));
    assert_eq!(capture["image"]["path"], path_argument(&out));
    assert_eq!(capture["image"]["uniform"], false, "the fixture window draws visible controls");

    let scale = backing_scale_factor();
    let bounds = &capture["window"]["bounds"];
    let expected = |edge: &str| (bounds[edge].as_f64().expect("bounds in points") * scale) as u64;
    assert_eq!(capture["image"]["width"].as_u64(), Some(expected("width")), "{capture}");
    assert_eq!(capture["image"]["height"].as_u64(), Some(expected("height")), "{capture}");
    assert_eq!(png_pixel_size(&out), (expected("width"), expected("height")));
}

#[test]
#[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted; run vp run test:ui"]
fn a_screenshot_without_a_window_captures_the_applications_main_window() {
    let fixture = Fixture::launch();
    let directory = TemporaryDirectory::new();
    let out = directory.file("default.png");

    let capture = run_ui(&["screenshot", fixture.target(), "--out", path_argument(&out)]);

    assert_eq!(capture["window"]["title"], MAIN_WINDOW_TITLE);
    assert_eq!(capture["window"]["id"].as_u64(), Some(main_window_id(&fixture)));
}

#[test]
#[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted; run vp run test:ui"]
fn a_screenshot_is_written_as_png_whatever_the_file_extension() {
    let fixture = Fixture::launch();
    let main_window = main_window_id(&fixture);
    let directory = TemporaryDirectory::new();
    let out = directory.file("capture.jpg");

    run_ui(&[
        "screenshot",
        fixture.target(),
        "--window",
        &main_window.to_string(),
        "--out",
        path_argument(&out),
    ]);

    png_pixel_size(&out);
}

#[test]
#[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted; run vp run test:ui"]
fn a_screenshot_never_overwrites_an_existing_output_file() {
    let fixture = Fixture::launch();
    let directory = TemporaryDirectory::new();
    let out = directory.file("taken.png");
    let existing = b"an earlier capture";
    std::fs::write(&out, existing).expect("write the existing file");

    let output = run_recon(&["ui", "screenshot", fixture.target(), "--out", path_argument(&out)]);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let error = reported_error(&output.stderr);
    assert_eq!(error["code"], "ALREADY_EXISTS");
    assert_eq!(error["details"]["path"], path_argument(&out));
    assert_eq!(std::fs::read(&out).expect("read the existing file"), existing);
}
