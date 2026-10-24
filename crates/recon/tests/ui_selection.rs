use std::path::Path;

use proptest::prelude::*;
use recon::application::Pid;
use recon::ui::instance::select_instance;
use recon::ui::window::{Bounds, Window, WindowId, select_window, title_from, windows_of};

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

#[test]
fn the_only_running_instance_is_selected_without_a_pid() {
    assert_eq!(select_instance(Path::new(BUNDLE), &[Pid(412)], None).unwrap(), Pid(412));
}

#[test]
fn a_bundle_that_is_not_running_is_never_launched() {
    let error = select_instance(Path::new(BUNDLE), &[], None).unwrap_err();
    assert_eq!(error, format!("{BUNDLE} is not running; recon ui does not launch applications"));

    let error = select_instance(Path::new(BUNDLE), &[], Some(Pid(412))).unwrap_err();
    assert_eq!(error, format!("{BUNDLE} is not running; recon ui does not launch applications"));
}

#[test]
fn several_instances_list_their_pids_and_name_the_pid_flag() {
    let error = select_instance(Path::new(BUNDLE), &[Pid(410), Pid(902)], None).unwrap_err();
    assert!(error.contains(BUNDLE), "{error}");
    assert!(error.contains("410"), "{error}");
    assert!(error.contains("902"), "{error}");
    assert!(error.contains("--pid"), "{error}");
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
    let error = select_instance(Path::new(BUNDLE), &running, Some(Pid(411))).unwrap_err();
    assert!(error.contains("411"), "{error}");
    assert!(error.contains(BUNDLE), "{error}");
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

    let error = select_window(windows.clone(), WindowId(5), Pid(42)).unwrap_err();
    assert_eq!(error, "window 5 is not a window of process 42");
    let error = select_window(windows, WindowId(77), Pid(42)).unwrap_err();
    assert_eq!(error, "window 77 is not a window of process 42");
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
        let error = select_instance(Path::new(BUNDLE), &pids, None).unwrap_err();
        for pid in &pids {
            prop_assert!(error.contains(&pid.to_string()), "{error} omits {pid}");
        }
        prop_assert!(error.contains("--pid"), "{error}");
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
