//! Choosing one window of a running instance from the window server's inventory. The rules are
//! about the records the window server reported, so these tests need no Mac.

use recon_core::error::{ErrorCode, ErrorReport, Failure};
use recon_core::process::Pid;
use recon_core::schema::output_schema;
use recon_window_server_macos::window::{
    Bounds, Window, WindowId, select_window, title_from, windows_of,
};
use serde_json::{Value, json};

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

/// Every failure a command reports is one envelope of the published shape.
fn details(failure: &Failure) -> Value {
    let schema = output_schema::<ErrorReport<'static>>();
    let validator = jsonschema::draft202012::new(&schema).expect("the envelope schema compiles");
    let envelope = serde_json::to_value(failure.report()).expect("the envelope encodes");
    let errors =
        validator.iter_errors(&envelope).map(|error| error.to_string()).collect::<Vec<_>>();
    assert!(errors.is_empty(), "{errors:?}: {envelope}");
    failure.details.clone().expect("the data needed to recover")
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
    }
}

#[test]
fn a_window_without_a_title_reports_no_title() {
    assert_eq!(title_from(None), None);
    assert_eq!(title_from(Some(String::new())), None);
    assert_eq!(title_from(Some("Untitled".to_owned())), Some("Untitled".to_owned()));
    assert_eq!(title_from(Some(" ".to_owned())), Some(" ".to_owned()));
}
