use std::path::Path;

use proptest::prelude::*;
use recon::application::Pid;
use recon::error::{ErrorCode, Failure};
use recon::ui::instance::select_instance;
use recon::ui::window::{Bounds, Window, WindowId, select_window, title_from, windows_of};
use serde_json::{Value, json};

mod support;

use support::validate_failure;

const BUNDLE: &str = "/Applications/Logic Pro.app";

fn window(id: u32, owner_pid: u32) -> Window {
    Window {
        id: WindowId(id),
        owner_pid: Pid(owner_pid),
        title: None,
        layer: 0,
        bounds: Bounds { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
        on_screen: true,
        alpha: 1.0,
    }
}

fn details(failure: &Failure) -> Value {
    failure.details.clone().expect("the data needed to recover")
}

#[test]
fn the_only_running_instance_is_selected_without_a_pid() {
    assert_eq!(select_instance(Path::new(BUNDLE), &[Pid(412)], None).unwrap(), Pid(412));
}

#[test]
fn a_bundle_that_is_not_running_is_never_launched() {
    for requested in [None, Some(Pid(412))] {
        let failure = select_instance(Path::new(BUNDLE), &[], requested).unwrap_err();

        assert_eq!(failure.code, ErrorCode::NotRunning);
        assert!(!failure.retryable);
        assert_eq!(details(&failure), json!({ "application": BUNDLE }));
        validate_failure(&failure, &["ui", "windows"]);
    }
}

#[test]
fn several_instances_are_an_ambiguous_instance_naming_every_process() {
    let failure = select_instance(Path::new(BUNDLE), &[Pid(410), Pid(902)], None).unwrap_err();

    assert_eq!(failure.code, ErrorCode::AmbiguousInstance);
    assert!(!failure.retryable);
    assert_eq!(details(&failure), json!({ "application": BUNDLE, "pids": [410, 902] }));
    validate_failure(&failure, &["ui", "windows"]);
}

#[test]
fn a_requested_pid_selects_that_instance() {
    let running = [Pid(410), Pid(902)];
    assert_eq!(select_instance(Path::new(BUNDLE), &running, Some(Pid(902))).unwrap(), Pid(902));
    assert_eq!(select_instance(Path::new(BUNDLE), &[Pid(410)], Some(Pid(410))).unwrap(), Pid(410));
}

#[test]
fn a_requested_pid_must_be_an_instance_of_the_resolved_bundle() {
    let running = [Pid(410), Pid(902)];

    let failure = select_instance(Path::new(BUNDLE), &running, Some(Pid(411))).unwrap_err();

    assert_eq!(failure.code, ErrorCode::NotFound);
    assert_eq!(
        details(&failure),
        json!({
            "kind": "instance",
            "selector": null,
            "path": BUNDLE,
            "pid": 411,
            "windowId": null,
            "runningPids": [410, 902],
            "detail": null,
        })
    );
    validate_failure(&failure, &["ui", "windows"]);
}

#[test]
fn the_windows_of_an_instance_exclude_other_processes_and_keep_their_order() {
    let all = vec![window(5, 90), window(3, 42), window(4, 90), window(1, 42)];
    let mine = windows_of(all, Pid(42));
    assert_eq!(mine.iter().map(|window| window.id).collect::<Vec<_>>(), [WindowId(3), WindowId(1)]);
}

#[test]
fn a_window_must_belong_to_the_selected_instance() {
    let windows = vec![window(3, 42), window(5, 90)];
    assert_eq!(select_window(windows.clone(), WindowId(3), Pid(42)).unwrap().id, WindowId(3));

    for unowned in [WindowId(5), WindowId(77)] {
        let failure = select_window(windows.clone(), unowned, Pid(42)).unwrap_err();

        assert_eq!(failure.code, ErrorCode::NotFound);
        assert!(!failure.retryable);
        assert_eq!(
            details(&failure),
            json!({
                "kind": "window",
                "selector": null,
                "path": null,
                "pid": 42,
                "windowId": unowned.0,
                "runningPids": null,
                "detail": null,
            })
        );
        validate_failure(&failure, &["ui", "tree"]);
    }
}

#[test]
fn a_window_without_a_title_reports_no_title() {
    assert_eq!(title_from(None), None);
    assert_eq!(title_from(Some(String::new())), None);
    assert_eq!(title_from(Some("Untitled".to_owned())), Some("Untitled".to_owned()));
    assert_eq!(title_from(Some(" ".to_owned())), Some(" ".to_owned()));
}

proptest! {
    #[test]
    fn several_instances_are_never_resolved_without_the_pid_flag(
        pids in prop::collection::hash_set(1u32..100_000, 2..12)
    ) {
        let pids = pids.into_iter().map(Pid).collect::<Vec<_>>();
        let failure = select_instance(Path::new(BUNDLE), &pids, None).unwrap_err();
        prop_assert_eq!(failure.code, ErrorCode::AmbiguousInstance);
        let reported = details(&failure);
        let reported = reported["pids"].as_array().expect("the running process ids");
        prop_assert_eq!(reported.len(), pids.len());
        for pid in &pids {
            prop_assert!(reported.contains(&json!(pid.0)), "{} omits {pid}", details(&failure));
        }
    }

    #[test]
    fn only_a_running_pid_is_ever_selected(
        pids in prop::collection::hash_set(1u32..1_000, 0..8),
        requested in 1u32..1_000,
    ) {
        let pids = pids.into_iter().map(Pid).collect::<Vec<_>>();
        let requested = Pid(requested);
        match select_instance(Path::new(BUNDLE), &pids, Some(requested)) {
            Ok(selected) => {
                prop_assert_eq!(selected, requested);
                prop_assert!(pids.contains(&requested));
            }
            Err(_) => prop_assert!(!pids.contains(&requested)),
        }
    }
}
