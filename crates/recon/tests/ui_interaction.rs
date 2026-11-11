#![cfg(target_os = "macos")]

//! The live evidence for proposal.0007: every supported interaction, against a real AppKit
//! target and a real Mac Catalyst target, while a foreground receiver keeps the user's input.
//!
//! Each check asserts three separate facts, because a command can satisfy one and fail another:
//! what the target's own handler did, read from the target's record; what Recon reported; and
//! what happened to the desktop, read continuously from outside Recon by the same watch the
//! no-focus evidence uses. A posted event that ran no handler, a handler that ran twice, a
//! handler that ran in the other window, and an operation that disturbed the person at the Mac
//! are all distinct failures here.
//!
//! Every test is opt-in: macOS grants Accessibility and event posting to one executable at one
//! path, the targets are built app bundles, and the run must be hands-off — the watch cannot
//! tell a person moving the mouse from a command that moved it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};

#[path = "ui_observation/harness.rs"]
#[allow(dead_code)]
mod harness;

#[path = "ui_observation/desktop.rs"]
#[allow(dead_code)]
mod desktop;

#[path = "ui_interaction/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;

#[path = "ui_interaction/interference.rs"]
#[allow(dead_code)]
mod interference;

use desktop::{Seen, Watch};
use fixtures::{App, control_point, coordinate, in_window, of_kind, wait_for};

// ── Click ────────────────────────────────────────────────────────────────────

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_an_appkit_element_click_presses_through_accessibility_and_through_the_mouse() {
    let desktop = Desktop::appkit();
    for label in ["main", "inspector"] {
        let window = desktop.window(label);
        let id = window.id();
        desktop.element(&window, "AXButton", "Record");

        let (response, events) = desktop.observed(&[
            "click",
            desktop.target.target(),
            "--window",
            &id,
            "--role",
            "AXButton",
            "--title",
            "Record",
        ]);

        assert_eq!(response["mechanism"], "ax-press", "{response}");
        assert_eq!(response["method"]["effective"], "ax", "{response}");
        assert_eq!(response["method"]["requested"], "auto", "{response}");
        assert_eq!(response["element"]["title"], "Record", "{response}");
        assert_eq!(response["outcome"], "unverified", "{response}");
        desktop.identity(&response, &window);
        assert_eq!(of_kind(&events, label, "ax-press").len(), 1, "{events:?}");
        assert_eq!(of_kind(&events, label, "action").len(), 1, "{events:?}");
        assert!(
            of_kind(&events, label, "mouse-down").is_empty(),
            "an accessibility press sent mouse events: {events:?}"
        );
        untouched(&events, label);

        let (response, events) = desktop.observed(&[
            "click",
            desktop.target.target(),
            "--window",
            &id,
            "--role",
            "AXButton",
            "--title",
            "Record",
            "--method",
            "mouse",
        ]);

        assert_eq!(response["mechanism"], "mouse-events", "{response}");
        assert_eq!(response["method"]["effective"], "mouse", "{response}");
        desktop.identity(&response, &window);
        let presses = of_kind(&events, label, "mouse-down");
        assert_eq!(presses.len(), 1, "{events:?}");
        assert_eq!(presses[0]["control"], "record", "{events:?}");
        assert_eq!(presses[0]["button"], "left", "{events:?}");
        assert_eq!(of_kind(&events, label, "action").len(), 1, "{events:?}");
        assert!(
            of_kind(&events, label, "ax-press").is_empty(),
            "a forced mouse click pressed through accessibility: {events:?}"
        );
        untouched(&events, label);
    }
}

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_an_appkit_coordinate_click_delivers_each_button_count_and_modifier_once() {
    let desktop = Desktop::appkit();
    for label in ["main", "inspector"] {
        let window = desktop.window(label);
        for click in Click::ALL {
            let events = desktop.clicked(&window, label, click);
            click.assert(&events, label);
            untouched(&events, label);
        }
    }
}

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_a_catalyst_element_click_presses_through_accessibility_and_through_the_mouse() {
    let desktop = Desktop::catalyst();
    for label in ["main", "inspector"] {
        let window = desktop.window(label);
        let id = window.id();
        desktop.identified(&window, "AXButton", "record");

        let (response, events) = desktop.observed(&[
            "click",
            desktop.target.target(),
            "--window",
            &id,
            "--role",
            "AXButton",
            "--identifier",
            "record",
        ]);

        assert_eq!(response["mechanism"], "ax-press", "{response}");
        assert_eq!(response["method"]["effective"], "ax", "{response}");
        desktop.identity(&response, &window);
        assert_eq!(of_kind(&events, label, "ax-press").len(), 1, "{events:?}");
        untouched(&events, label);

        let (response, events) = desktop.observed(&[
            "click",
            desktop.target.target(),
            "--window",
            &id,
            "--role",
            "AXButton",
            "--identifier",
            "record",
            "--method",
            "mouse",
        ]);

        assert_eq!(response["mechanism"], "mouse-events", "{response}");
        desktop.identity(&response, &window);
        // A Catalyst button ends a mouse press in its action without calling `touchesBegan`, so
        // the window's own hit test for a primary-button touch is what shows a mouse reached it,
        // and the absence of an accessibility press shows nothing else did.
        assert!(
            of_kind(&events, label, "hit-test").iter().any(|event| event["buttonMask"] == 1),
            "no primary-button touch reached the window: {events:?}"
        );
        assert!(of_kind(&events, label, "ax-press").is_empty(), "{events:?}");
        assert_eq!(of_kind(&events, label, "action").len(), 1, "{events:?}");
        untouched(&events, label);
    }
}

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_a_catalyst_click_over_a_control_delivers_each_button_count_and_modifier_once() {
    let desktop = Desktop::catalyst();
    for label in ["main", "inspector"] {
        let window = desktop.window(label);
        // Mac Catalyst never hands a middle click to UIKit, so the matrix leaves it out here.
        for click in Click::ALL.into_iter().filter(|click| click.button != "middle") {
            let events = desktop.clicked(&window, label, click);
            click.assert(&events, label);
            untouched(&events, label);
        }
    }
}

// ── Scroll ───────────────────────────────────────────────────────────────────

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_appkit_scrolling_moves_both_axes_both_directions_and_both_units() {
    let desktop = Desktop::appkit();
    let window = desktop.window("main");
    desktop.parked(&window, "main");
    for region in ["scroll-outer", "scroll-nested"] {
        for unit in ["pixel", "line"] {
            for scroll in Scroll::both_axes(unit) {
                let (previous, events) = desktop.scrolled(&window, region, scroll);
                scroll.assert_wheel(&previous, &events, "main", region);
                untouched(&events, "main");
            }
        }
    }

    // The second window of the same process scrolls on its own, and nothing crosses between
    // them: the region that moved is the one in the window that was addressed.
    let inspector = desktop.window("inspector");
    let scroll = Scroll { dx: 0, dy: 180, unit: "pixel" };
    let (previous, events) = desktop.scrolled(&inspector, "scroll-outer", scroll);
    scroll.assert_wheel(&previous, &events, "inspector", "scroll-outer");
    untouched(&events, "inspector");
}

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_catalyst_scrolling_moves_both_axes_both_directions_and_both_units() {
    let desktop = Desktop::catalyst();
    let window = desktop.window("main");
    for region in ["scroll-outer", "scroll-nested"] {
        for unit in ["pixel", "line"] {
            for scroll in Scroll::both_axes(unit) {
                let (previous, events) = desktop.scrolled(&window, region, scroll);
                scroll.assert_moved(&previous, &events, "main", region);
                untouched(&events, "main");
            }
        }
    }

    let inspector = desktop.window("inspector");
    let scroll = Scroll { dx: 0, dy: 180, unit: "pixel" };
    let (previous, events) = desktop.scrolled(&inspector, "scroll-outer", scroll);
    scroll.assert_moved(&previous, &events, "inspector", "scroll-outer");
    untouched(&events, "inspector");
}

// ── Key chords ───────────────────────────────────────────────────────────────

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_an_appkit_chord_runs_only_the_selected_windows_action_once() {
    let desktop = Desktop::appkit();
    let mut mechanisms = Vec::new();
    for label in ["main", "inspector"] {
        let window = desktop.window(label);
        let id = window.id();

        let (response, events) =
            desktop.observed(&["key", desktop.target.target(), "cmd+b", "--window", &id]);

        assert_eq!(response["mechanism"], "key-events", "{response}");
        assert_eq!(response["chord"]["key"], "b", "{response}");
        assert_eq!(response["chord"]["modifiers"], json!(["cmd"]), "{response}");
        assert_eq!(response["keyWindow"]["after"].as_u64(), Some(window.id), "{response}");
        let mechanism = response["keyWindow"]["mechanism"].as_str().unwrap_or_default().to_owned();
        assert!(
            matches!(mechanism.as_str(), "window-server-records" | "already-key"),
            "an AppKit chord took its key window through {mechanism}: {response}"
        );
        mechanisms.push(mechanism);
        desktop.identity(&response, &window);

        let chords = of_kind(&events, label, "chord");
        assert_eq!(chords.len(), 1, "the selected window's action ran {} times", chords.len());
        untouched(&events, label);
        released(&events, label);
    }
    // The two windows cannot both have been the target's key window already: addressing them in
    // turn is what makes the external key-window change the mechanism under test here.
    assert!(
        mechanisms.iter().any(|mechanism| mechanism == "window-server-records"),
        "neither chord changed the target's key window from outside: {mechanisms:?}"
    );
}

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_a_catalyst_chord_runs_through_a_session_with_balanced_press_begin_and_end() {
    let desktop = Desktop::catalyst();
    let session = desktop.session();
    let mut mechanisms = Vec::new();
    for label in ["main", "inspector"] {
        let window = desktop.window(label);
        let id = window.id();

        let (response, events) = desktop.observed(&[
            "key",
            desktop.target.target(),
            "cmd+b",
            "--window",
            &id,
            "--session",
            &session,
        ]);

        assert_eq!(response["mechanism"], "key-events", "{response}");
        assert_eq!(response["keyWindow"]["after"].as_u64(), Some(window.id), "{response}");
        let mechanism = response["keyWindow"]["mechanism"].as_str().unwrap_or_default().to_owned();
        assert!(
            matches!(mechanism.as_str(), "uikit-helper" | "already-key"),
            "a Catalyst chord took its key window through {mechanism}: {response}"
        );
        mechanisms.push(mechanism);
        desktop.identity(&response, &window);

        assert_eq!(of_kind(&events, label, "chord").len(), 1, "{events:?}");
        let begins = of_kind(&events, label, "press-begin");
        let ends = of_kind(&events, label, "press-end");
        assert_eq!(begins.len(), ends.len(), "a press that began and never ended: {events:?}");
        for (kind, events) in [("begin", begins), ("end", ends)] {
            let base = events.iter().filter(|event| event["keyCode"] == 5).count();
            assert_eq!(base, 1, "the base key had {base} press-{kind} events: {events:?}");
        }
        untouched(&events, label);
    }
    // At most one of the two windows can have been UIKit's key window already, so the other one
    // proves the helper changed it inside the target.
    assert!(
        mechanisms.iter().any(|mechanism| mechanism == "uikit-helper"),
        "neither chord went through the helper's make-window-key: {mechanisms:?}"
    );
    harness::run_ui(&["session", "close", &session]);
}

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_a_catalyst_chord_without_a_session_is_refused_and_opens_none() {
    let desktop = Desktop::catalyst();
    let window = desktop.window("main");
    let id = window.id();
    let mark = desktop.target.evidence().sequence();

    let error =
        refused(&["key", desktop.target.target(), "cmd+b", "--window", &id], "ACTION_UNAVAILABLE");

    assert!(
        error["message"].as_str().is_some_and(|message| message.contains("session")),
        "the refusal has to name the session requirement: {error}"
    );
    assert!(
        desktop.target.settled().since(mark).is_empty(),
        "a refused chord reached the target anyway"
    );
}

// ── Refusals that protect the person at the Mac ──────────────────────────────

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_a_frontmost_target_refuses_what_would_move_the_users_input() {
    let desktop = Desktop::appkit();
    let main = desktop.window("main");
    let inspector = desktop.window("inspector");

    // The person at the Mac is now using the target itself, which is the one case where a
    // mouse click or a chord in another of its windows would move their keyboard focus.
    interference::activate(desktop.target.target());
    wait_for(|| (harness::frontmost_pid() == Some(desktop.target.pid)).then_some(()))
        .expect("the target did not come to the front");
    let key = key_window(&desktop.target).expect("the target reports which window is key");
    let (key_window, other) = if key == "main" { (&main, &inspector) } else { (&inspector, &main) };
    let other_id = other.id();
    let (x, y) = other.point("canvas");
    let target = desktop.target.target();

    refused(&["click", target, "--window", &other_id, "--x", &x, "--y", &y], "ACTION_UNAVAILABLE");
    refused(&["key", target, "cmd+b", "--window", &other_id], "ACTION_UNAVAILABLE");

    // An accessibility press and a wheel event move nobody's focus, so they stay available.
    let response =
        acted(&["click", target, "--window", &other_id, "--role", "AXButton", "--title", "Record"]);
    assert_eq!(response["mechanism"], "ax-press", "{response}");
    let (scroll_x, scroll_y) = other.point("scroll-outer");
    acted(&[
        "scroll", target, "--window", &other_id, "--x", &scroll_x, "--y", &scroll_y, "--dy", "120",
    ]);

    // In the window that already has the target's keyboard focus, a mouse click takes nothing
    // from anyone.
    let key_id = key_window.id();
    let (key_x, key_y) = key_window.point("canvas");
    let response = acted(&["click", target, "--window", &key_id, "--x", &key_x, "--y", &key_y]);
    assert_eq!(response["mechanism"], "mouse-events", "{response}");

    interference::activate(desktop.receiver.target());
}

#[test]
#[ignore = "needs target/signed/recon with Accessibility and event posting granted, the built fixtures, and a hands-off Mac; run mise run test:interaction"]
fn proposal0007_an_operation_that_names_the_wrong_window_or_element_sends_nothing() {
    let desktop = Desktop::appkit();
    let window = desktop.window("main");
    let id = window.id();
    let target = desktop.target.target();
    let (x, y) = window.point("canvas");
    let mark = desktop.target.evidence().sequence();

    // A window of another process, named on this one.
    let foreign = desktop.receiver_window().to_string();
    let error =
        refused(&["click", target, "--window", &foreign, "--x", &x, "--y", &y], "NOT_FOUND");
    assert_eq!(error["details"]["kind"], "window", "{error}");

    // Selectors that do not name exactly one live, clickable element.
    refused(
        &["click", target, "--window", &id, "--role", "AXButton", "--title", "Twin"],
        "AMBIGUOUS_ELEMENT",
    );
    refused(
        &["click", target, "--window", &id, "--role", "AXButton", "--title", "Disabled"],
        "ACTION_UNAVAILABLE",
    );
    refused(
        &["click", target, "--window", &id, "--role", "AXButton", "--title", "Absent"],
        "NOT_FOUND",
    );

    // A point outside the window, and options that contradict the method.
    let width = coordinate(window.bounds["width"].as_f64().expect("a window width"));
    refused(&["click", target, "--window", &id, "--x", &width, "--y", &y], "ACTION_UNAVAILABLE");
    refused(
        &["click", target, "--window", &id, "--x", &x, "--y", &y, "--method", "ax"],
        "INVALID_ARGUMENT",
    );
    refused(
        &[
            "click", target, "--window", &id, "--role", "AXButton", "--title", "Record",
            "--method", "ax", "--button", "right",
        ],
        "INVALID_ARGUMENT",
    );
    // An element that advertises no press cannot be pressed through accessibility, and Recon
    // never falls back to the mouse.
    refused(
        &["click", target, "--window", &id, "--role", "AXTextField", "--method", "ax"],
        "ACTION_UNAVAILABLE",
    );

    assert!(
        desktop.target.settled().since(mark).is_empty(),
        "a refused operation reached the target anyway"
    );
}

// ── Negative controls: the guard has to report a disturbed desktop ───────────

#[test]
#[ignore = "disturbs the desktop deliberately; needs target/signed/recon, the built fixtures, and a hands-off Mac"]
fn proposal0007_activating_another_application_during_an_operation_fails_it() {
    let desktop = Desktop::appkit();
    let window = desktop.window("main");
    let id = window.id();
    let target = desktop.target.target().to_owned();
    let receiver = desktop.receiver.target().to_owned();

    let error = disturbed(&["key", &target, "cmd+b", "--window", &id], || {
        interference::activate_and_restore(&target, &receiver);
    });

    assert_eq!(error["details"]["evidenceSource"], "foreground-application", "{error}");
    interference::activate(desktop.receiver.target());
}

#[test]
#[ignore = "disturbs the desktop deliberately; needs target/signed/recon, the built fixtures, and a hands-off Mac"]
fn proposal0007_an_accessibility_raise_of_a_target_window_during_an_operation_fails_it() {
    let desktop = Desktop::appkit();
    let window = desktop.window("main");
    let id = window.id();
    let pid = desktop.target.pid;

    let error = disturbed(&["key", desktop.target.target(), "cmd+b", "--window", &id], || {
        interference::ax_raise(pid, u32::try_from(window.id).unwrap());
    });

    assert_eq!(error["details"]["evidenceSource"], "key-window", "{error}");
    interference::activate(desktop.receiver.target());
}

#[test]
#[ignore = "disturbs the desktop deliberately; needs target/signed/recon, the built fixtures, and a hands-off Mac"]
fn proposal0007_warping_the_pointer_during_an_operation_fails_it() {
    let desktop = Desktop::appkit();
    let window = desktop.window("main");
    let id = window.id();

    let error = disturbed(&["key", desktop.target.target(), "cmd+b", "--window", &id], || {
        interference::warp_pointer_and_restore();
    });

    assert_eq!(error["details"]["evidenceSource"], "pointer-location", "{error}");
}

#[test]
#[ignore = "disturbs the desktop deliberately; needs two Spaces on the main display, target/signed/recon, and the built fixtures"]
fn proposal0007_switching_space_during_an_operation_fails_it() {
    assert!(
        interference::Spaces::of_this_mac().is_some(),
        "this Mac reports one Space on its main display; add a second desktop in Mission \
         Control, or this control cannot be exercised"
    );
    let desktop = Desktop::appkit();
    let window = desktop.window("main");
    let id = window.id();

    let error = disturbed(&["key", desktop.target.target(), "cmd+b", "--window", &id], || {
        interference::Spaces::of_this_mac().expect("two Spaces").switch_and_return();
    });

    assert!(
        matches!(
            error["details"]["evidenceSource"].as_str(),
            Some("space-change" | "foreground-application")
        ),
        "{error}"
    );
}

#[test]
#[ignore = "posts a keystroke into the session; needs target/signed/recon, the built fixtures, and a hands-off Mac"]
fn proposal0007_a_session_level_keystroke_during_an_operation_fails_it() {
    assert!(
        interference::can_post_events(),
        "the process running these tests may not post events, so it cannot divert input; grant \
         it Accessibility"
    );
    let desktop = Desktop::appkit();
    let window = desktop.window("main");
    let id = window.id();
    let watched = desktop.receiver.evidence().sequence();

    let error = disturbed(&["key", desktop.target.target(), "cmd+b", "--window", &id], || {
        interference::session_keystroke();
    });

    assert_eq!(error["details"]["evidenceSource"], "input-routing", "{error}");
    // The control has to have reached the session rather than only the counters: the
    // application the person is using received the keystroke.
    let received = desktop.receiver.settled().since(watched);
    assert!(
        !of_kind(&received, "front", "key-down").is_empty(),
        "the control posted nothing into the session: {received:?}"
    );
}

#[test]
#[ignore = "changes the frontmost application's key window; needs target/signed/recon, the built fixtures, and a hands-off Mac"]
fn proposal0007_a_key_window_change_in_the_frontmost_application_during_an_operation_fails_it() {
    let desktop = Desktop::appkit();
    let window = desktop.window("main");
    let id = window.id();

    let error = disturbed(&["key", desktop.target.target(), "cmd+b", "--window", &id], || {
        desktop.receiver.perform("key-window-secondary");
    });

    assert_eq!(error["details"]["evidenceSource"], "key-window", "{error}");
    desktop.receiver.perform("key-window-front");
}

#[test]
#[ignore = "needs a Mac that counts a posted HID event as hardware input, target/signed/recon, and the built fixtures"]
fn proposal0007_hardware_input_during_an_unrelated_change_is_not_blamed_on_the_target() {
    assert!(
        interference::can_post_events(),
        "the process running these tests may not post events; grant it Accessibility"
    );
    assert!(
        interference::hardware_keystroke(),
        "this Mac does not count a posted event as hardware input, so this control cannot be \
         produced synthetically; exercise it by typing on the keyboard during an operation"
    );
    let desktop = Desktop::appkit();
    let window = desktop.window("main");
    let id = window.id();
    let pid = desktop.target.pid;

    let (output, seen) = ran(&["key", desktop.target.target(), "cmd+b", "--window", &id], || {
        interference::hardware_keystroke();
        desktop.receiver.perform("key-window-secondary");
    });

    assert!(seen.hardware_input > 0, "the control produced no hardware input: {seen:?}");
    if !output.status.success() {
        let error = reported_error(&output.stderr);
        assert_eq!(error["code"], "UI_INTERFERENCE", "{error}");
        assert_ne!(
            error["details"]["pid"].as_u64(),
            Some(u64::from(pid)),
            "a change made during hardware input was blamed on the target: {error}"
        );
    }
    desktop.receiver.perform("key-window-front");
}

// ── The desktop one row of the evidence runs on ──────────────────────────────

/// A target and the application that keeps the user's place while it is acted on.
///
/// The receiver is launched last and activates itself, so every operation above runs against a
/// background target whose windows are covered by the application the person at the Mac is
/// using, with the keyboard focus somewhere else entirely.
struct Desktop {
    target: App,
    receiver: App,
}

impl Desktop {
    fn appkit() -> Desktop {
        Desktop::of(App::appkit(), "appkit")
    }

    fn catalyst() -> Desktop {
        Desktop::of(App::catalyst(), "catalyst")
    }

    /// `toolkit` is what the target itself reports building against, so a Catalyst row cannot
    /// quietly become an AppKit run: the record says which framework drew the windows, and the
    /// bundle it came from was built for that platform.
    fn of(target: App, toolkit: &str) -> Desktop {
        assert_eq!(
            target.evidence().toolkit(),
            toolkit,
            "the launched target does not report itself as the {toolkit} fixture"
        );
        let receiver = App::receiver();
        wait_for(|| (harness::frontmost_pid() == Some(receiver.pid)).then_some(()))
            .unwrap_or_else(|| {
                panic!(
                    "the foreground receiver did not come to front: receiver pid {}, frontmost {:?}",
                    receiver.pid,
                    harness::frontmost_pid()
                )
            });
        Desktop { target, receiver }
    }

    /// One instrumented window, bound to the window the window server reports: the fixture
    /// published the window number it drew its controls in, and that number is the one every
    /// command addresses.
    fn window(&self, label: &str) -> Window {
        let evidence = self.target.evidence();
        let published = evidence.window(label).clone();
        let id = published["windowId"].as_u64().expect("a published window number");
        assert!(id > 0, "{published}");
        let observation = harness::run_ui(&["windows", self.target.target()]);
        let reported = observation["windows"]
            .as_array()
            .expect("a windows array")
            .iter()
            .find(|window| window["id"].as_u64() == Some(id))
            .unwrap_or_else(|| {
                panic!("the window server does not report window {id} of the target: {observation}")
            });
        assert_eq!(reported["title"], published["title"], "{reported} against {published}");
        Window { id, bounds: reported["bounds"].clone(), published }
    }

    /// A window of the receiver, which belongs to another process entirely.
    fn receiver_window(&self) -> u64 {
        let observation = harness::run_ui(&["windows", self.receiver.target()]);
        observation["windows"].as_array().expect("a windows array")[0]["id"]
            .as_u64()
            .expect("a window id")
    }

    /// An open session on this target, for the one operation that needs the helper.
    fn session(&self) -> String {
        let opened =
            harness::run_ui(&["session", "open", self.target.target(), "--mode", "attach"]);
        assert_eq!(opened["toolkit"], "catalyst", "{opened}");
        assert_eq!(opened["pid"].as_u64(), Some(u64::from(self.target.pid)), "{opened}");
        opened["sessionId"].as_str().expect("a session identifier").to_owned()
    }

    /// The element a selector resolves to, read before acting. A selector that matches nothing
    /// would fail the lookup inside Recon; this says what the target actually publishes.
    fn element(&self, window: &Window, role: &str, title: &str) {
        let id = window.id();
        let tree = harness::run_ui(&["ax", "tree", self.target.target(), "--window", &id]);
        let found = harness::nodes(&tree["root"])
            .into_iter()
            .filter(|node| node["role"] == role && node["title"] == title)
            .count();
        assert_eq!(found, 1, "{found} elements are {role} titled {title}: {tree}");
    }

    /// The element an identifier selector resolves to, read before acting.
    fn identified(&self, window: &Window, role: &str, identifier: &str) {
        let id = window.id();
        let tree = harness::run_ui(&["ax", "tree", self.target.target(), "--window", &id]);
        let found = harness::nodes(&tree["root"])
            .into_iter()
            .filter(|node| node["role"] == role && node["identifier"] == identifier)
            .count();
        assert_eq!(found, 1, "{found} elements are {role} identified {identifier}: {tree}");
    }

    /// Returns a window's outer scroll region to its launch offset. The nested region is
    /// published at launch, so an outer region left scrolled by an earlier test would move it
    /// away from the point that addresses it.
    fn parked(&self, window: &Window, label: &str) {
        let outer = |events: &[Value]| -> Vec<(f64, f64)> {
            of_kind(events, label, "scroll-offset")
                .into_iter()
                .filter(|event| event["control"] == "scroll-outer")
                .map(|event| {
                    (
                        event["offsetX"].as_f64().expect("an offset"),
                        event["offsetY"].as_f64().expect("an offset"),
                    )
                })
                .collect()
        };
        let offsets = outer(self.target.settled().events());
        let launch = *offsets.first().expect("scroll-outer has a launch offset");
        let now = *offsets.last().expect("scroll-outer has a current offset");
        if launch == now {
            return;
        }
        let id = window.id();
        let (x, y) = window.point("scroll-outer");
        let dx = coordinate(launch.0 - now.0);
        let dy = coordinate(launch.1 - now.1);
        acted(&[
            "scroll",
            self.target.target(),
            "--window",
            &id,
            "--x",
            &x,
            "--y",
            &y,
            "--dx",
            &dx,
            "--dy",
            &dy,
            "--unit",
            "pixel",
        ]);
        let offsets = outer(self.target.settled().events());
        assert_eq!(
            offsets.last().copied(),
            Some(launch),
            "scroll-outer did not return to its launch offset"
        );
    }

    /// Runs one operation and returns what Recon reported and what the target recorded while it
    /// ran. The receiver's record must gain nothing: the person's application neither received
    /// input nor lost its place.
    fn observed(&self, arguments: &[&str]) -> (Value, Vec<Value>) {
        let mark = self.target.evidence().sequence();
        let watched = self.receiver.evidence().sequence();
        let response = acted(arguments);
        let events = self.target.settled().since(mark);
        let user = self.receiver.settled().since(watched);
        assert!(user.is_empty(), "the application the person is using recorded {user:?}");
        (response, events)
    }

    /// One coordinate click on a window's canvas.
    fn clicked(&self, window: &Window, label: &str, click: Click) -> Vec<Value> {
        let id = window.id();
        let (x, y) = window.point("canvas");
        let mut arguments =
            vec!["click", self.target.target(), "--window", &id, "--x", &x, "--y", &y];
        arguments.extend_from_slice(click.arguments);

        let (response, events) = self.observed(&arguments);

        assert_eq!(response["mechanism"], "mouse-events", "{response}");
        assert_eq!(response["button"], click.button, "{response}");
        assert_eq!(response["count"].as_u64(), Some(u64::from(click.count)), "{response}");
        assert_eq!(response["modifiers"], json!(click.modifiers), "{response}");
        assert_eq!(response["method"]["effective"], "mouse", "{response}");
        assert_eq!(response["method"]["reason"], "mouse-required", "{response}");
        self.identity(&response, window);
        assert!(
            in_window(&events, label).iter().any(|event| event["control"] == "canvas"),
            "nothing reached the canvas: {events:?}"
        );
        events
    }

    /// One wheel event at a window's scroll region.
    fn scrolled(&self, window: &Window, region: &str, scroll: Scroll) -> (Vec<Value>, Vec<Value>) {
        let id = window.id();
        let (x, y) = window.point(region);
        let dx = scroll.dx.to_string();
        let dy = scroll.dy.to_string();

        let previous = self.target.evidence().events().to_vec();
        let (response, events) = self.observed(&[
            "scroll",
            self.target.target(),
            "--window",
            &id,
            "--x",
            &x,
            "--y",
            &y,
            "--dx",
            &dx,
            "--dy",
            &dy,
            "--unit",
            scroll.unit,
        ]);

        assert_eq!(response["mechanism"], "scroll-wheel", "{response}");
        assert_eq!(response["delta"]["x"].as_i64(), Some(i64::from(scroll.dx)), "{response}");
        assert_eq!(response["delta"]["y"].as_i64(), Some(i64::from(scroll.dy)), "{response}");
        assert_eq!(response["delta"]["unit"], scroll.unit, "{response}");
        self.identity(&response, window);
        (previous, events)
    }

    /// The process and the window Recon reports are the ones the record came from.
    fn identity(&self, response: &Value, window: &Window) {
        assert_eq!(response["application"]["path"], self.target.target(), "{response}");
        assert_eq!(
            response["application"]["pid"].as_u64(),
            Some(u64::from(self.target.pid)),
            "{response}"
        );
        assert!(response["application"]["pidversion"].as_u64().is_some(), "{response}");
        assert_eq!(response["window"].as_u64(), Some(window.id), "{response}");
        assert_eq!(self.target.evidence().pid(), self.target.pid, "another process answered");
    }
}

/// One instrumented window: what the fixture published about it, and what the window server
/// reports.
struct Window {
    id: u64,
    bounds: Value,
    published: Value,
}

impl Window {
    fn id(&self) -> String {
        self.id.to_string()
    }

    /// The centre of a published control, in the window-local points Recon takes.
    fn point(&self, control: &str) -> (String, String) {
        let (x, y) = control_point(&self.published, &self.bounds, control);
        (coordinate(x), coordinate(y))
    }
}

/// One click of the matrix: a button, a count and a modifier set, each of which must produce its
/// own effect exactly once.
#[derive(Clone, Copy)]
struct Click {
    button: &'static str,
    count: u8,
    modifiers: &'static [&'static str],
    arguments: &'static [&'static str],
}

impl Click {
    const ALL: [Click; 5] = [
        Click { button: "left", count: 1, modifiers: &[], arguments: &[] },
        Click { button: "right", count: 1, modifiers: &[], arguments: &["--button", "right"] },
        Click { button: "middle", count: 1, modifiers: &[], arguments: &["--button", "middle"] },
        Click { button: "left", count: 2, modifiers: &[], arguments: &["--count", "2"] },
        Click {
            button: "left",
            count: 1,
            modifiers: &["shift"],
            arguments: &["--modifiers", "shift"],
        },
    ];

    /// The target's own handler ran once for this click, with the button, the count and the
    /// modifiers that were asked for.
    fn assert(self, events: &[Value], label: &str) {
        let presses: Vec<&Value> = of_kind(events, label, "mouse-down")
            .into_iter()
            .filter(|event| event["control"] == "canvas")
            .collect();
        if self.button == "right" && presses.is_empty() {
            // UIKit delivers a secondary click to a context-menu interaction rather than as a
            // touch; either is the fixture's own handler for a right click, and neither may run
            // more than once.
            let menus = of_kind(events, label, "context-menu");
            assert_eq!(menus.len(), 1, "a right click ran {} handlers: {events:?}", menus.len());
            return;
        }
        assert_eq!(
            presses.len(),
            usize::from(self.count),
            "a {} click of count {} ran {} handlers: {events:?}",
            self.button,
            self.count,
            presses.len()
        );
        for (index, press) in presses.iter().enumerate() {
            assert_eq!(press["button"], self.button, "{press}");
            assert_eq!(press["modifiers"], json!(self.modifiers), "{press}");
            if self.count == 2 {
                assert_eq!(
                    press["count"].as_u64(),
                    Some(index as u64 + 1),
                    "a double click arrives as click states 1 and 2: {press}"
                );
            }
        }
    }
}

/// One wheel event of the matrix.
#[derive(Clone, Copy)]
struct Scroll {
    dx: i32,
    dy: i32,
    unit: &'static str,
}

impl Scroll {
    /// Both axes in both directions, at a magnitude the region can absorb in either unit.
    fn both_axes(unit: &'static str) -> [Scroll; 4] {
        let step = if unit == "line" { 3 } else { 180 };
        [
            Scroll { dx: 0, dy: step, unit },
            Scroll { dx: 0, dy: -step, unit },
            Scroll { dx: step, dy: 0, unit },
            Scroll { dx: -step, dy: 0, unit },
        ]
    }

    /// The wheel event reached the addressed region in the unit it was sent in, and the region
    /// moved the way the sign says.
    fn assert_wheel(self, previous: &[Value], events: &[Value], label: &str, region: &str) {
        let wheels: Vec<&Value> = of_kind(events, label, "scroll")
            .into_iter()
            .filter(|event| event["control"] == region)
            .collect();
        assert_eq!(wheels.len(), 1, "{region} received {} wheel events: {events:?}", wheels.len());
        assert_eq!(
            wheels[0]["precise"].as_bool(),
            Some(self.unit == "pixel"),
            "a {} unit arrived as precise={}: {}",
            self.unit,
            wheels[0]["precise"],
            wheels[0]
        );
        self.assert_moved(previous, events, label, region);
    }

    /// The addressed region's content moved in the direction the sign names: a positive delta
    /// advances the content right or down, whatever the natural-scrolling preference is.
    fn assert_moved(self, previous: &[Value], events: &[Value], label: &str, region: &str) {
        let offsets: Vec<&Value> = of_kind(events, label, "scroll-offset")
            .into_iter()
            .filter(|event| event["control"] == region)
            .collect();
        assert!(!offsets.is_empty(), "{region} did not move: {events:?}");
        let axis = if self.dy != 0 { "offsetY" } else { "offsetX" };
        let delta = if self.dy != 0 { self.dy } else { self.dx };
        let first = previous
            .iter()
            .rev()
            .find(|event| {
                event["window"] == label
                    && event["kind"] == "scroll-offset"
                    && event["control"] == region
            })
            .unwrap_or_else(|| panic!("{region} had no baseline offset"))
            .get(axis)
            .and_then(Value::as_f64)
            .expect("a baseline offset");
        let last = offsets[offsets.len() - 1][axis].as_f64().expect("an offset");
        if delta > 0 {
            assert!(last > first, "a positive {axis} delta did not advance it: {first} -> {last}");
        } else {
            assert!(
                last < first,
                "a negative {axis} delta did not move it back: {first} -> {last}"
            );
        }
    }
}

// ── Reading the records ──────────────────────────────────────────────────────

/// Nothing reached the window that was not addressed. A key-window change there is the target's
/// own business: the selected window becoming key is what makes the other one stop being key.
fn untouched(events: &[Value], label: &str) {
    let other = if label == "main" { "inspector" } else { "main" };
    let acted: Vec<&Value> = in_window(events, other)
        .into_iter()
        .filter(|event| event["kind"] != "key-window")
        .collect();
    assert!(acted.is_empty(), "the window that was not addressed acted: {acted:?}");
}

/// The chord was released: the base key came up carrying no modifier, and the target was left
/// holding none. A chord that left Command down would repeat its action instead.
fn released(events: &[Value], label: &str) {
    let ups = of_kind(events, label, "key-up");
    assert!(
        ups.iter().any(|event| {
            event["key"] == "b"
                && event["modifiers"].as_array().is_some_and(|modifiers| modifiers.is_empty())
        }),
        "no key up without modifiers reached the window: {events:?}"
    );
    if let Some(last) = of_kind(events, label, "flags").last() {
        assert_eq!(last["modifiers"], json!([]), "a modifier was left held: {events:?}");
    }
}

/// The window the target itself says is its key window.
fn key_window(app: &App) -> Option<String> {
    let evidence = app.settled();
    let last = evidence.events().iter().rev().find(|event| event["kind"] == "key-window")?.clone();
    (last["isKey"] == true).then(|| last["window"].as_str().unwrap_or_default().to_owned())
}

// ── Running the signed executable ────────────────────────────────────────────

/// Runs one `recon ui` command while a continuous watch reads the Mac, and holds it to the bar
/// every interaction shares: the command succeeded, and the person at the Mac kept their
/// frontmost application, their window order, their Space, their pointer and their input.
fn acted(arguments: &[&str]) -> Value {
    let watch = Watch::start();
    let output = harness::run_recon(&invocation(arguments));
    let seen = watch.stop();
    assert!(
        output.status.success(),
        "recon ui {} failed: {}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(seen.samples > 2, "the watch has to span the command, not bracket it: {seen:?}");
    assert!(seen.undisturbed(), "recon ui {} disturbed the desktop: {seen:?}", arguments.join(" "));
    serde_json::from_slice(&output.stdout).expect("recon prints one JSON object on success")
}

/// Runs a command that must be refused, and returns the error it reported. A refusal sends no
/// input, so the desktop must be as quiet as it is for a success.
fn refused(arguments: &[&str], code: &str) -> Value {
    let watch = Watch::start();
    let output = harness::run_recon(&invocation(arguments));
    let seen = watch.stop();
    assert!(
        !output.status.success(),
        "recon ui {} was expected to fail with {code}: {}",
        arguments.join(" "),
        String::from_utf8_lossy(&output.stdout)
    );
    let error = reported_error(&output.stderr);
    assert_eq!(error["code"], code, "{error}");
    assert!(seen.undisturbed(), "a refusal disturbed the desktop: {seen:?}");
    error
}

/// Runs a command while a deliberate disturbance happens on the desktop, and returns the
/// interference Recon reported. The desktop checks are deliberately absent here: the control
/// moves the desktop, and the point is that Recon notices.
fn disturbed(arguments: &[&str], control: impl FnOnce() + Send) -> Value {
    let (output, seen) = ran(arguments, control);
    assert!(
        !output.status.success(),
        "the operation reported success while the desktop moved under it: {seen:?}"
    );
    let error = reported_error(&output.stderr);
    assert_eq!(error["code"], "UI_INTERFERENCE", "{error}");
    assert!(declared(arguments, &error), "{error}");
    assert_eq!(error["details"]["stage"], "interaction", "{error}");
    error
}

/// Runs a command with something else happening at the same time, and returns both what Recon
/// printed and what the watch saw.
fn ran(arguments: &[&str], control: impl FnOnce() + Send) -> (Output, Seen) {
    let watch = Watch::start();
    let words = invocation(arguments);
    let output = thread::scope(|scope| {
        scope.spawn(|| {
            // Late enough that Recon has a baseline, early enough to fall inside the interval
            // it watches.
            thread::sleep(Duration::from_millis(60));
            control();
        });
        Command::new(signed_recon()).args(&words).output().expect("start recon")
    });
    (output, watch.stop())
}

fn signed_recon() -> PathBuf {
    if let Some(recon) = std::env::var_os("RECON_UI_BINARY") {
        return PathBuf::from(recon);
    }
    let recon = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/signed/recon");
    assert!(recon.is_file(), "{} is missing; build it with `mise run recon:sign`", recon.display());
    recon
}

fn invocation<'a>(arguments: &[&'a str]) -> Vec<&'a str> {
    let mut invocation = vec!["ui"];
    invocation.extend_from_slice(arguments);
    invocation
}

/// Whether the code the command reported is one the registry publishes for it.
fn declared(arguments: &[&str], error: &Value) -> bool {
    let words: Vec<String> = invocation(arguments).iter().map(|word| (*word).to_owned()).collect();
    let Some(command) = recon::cli::resolved_command(&words) else {
        return false;
    };
    recon::cli::registry::registry().iter().find(|entry| entry.path == command).is_some_and(
        |entry| {
            entry
                .errors
                .iter()
                .any(|code| serde_json::to_value(code).expect("a code") == error["code"])
        },
    )
}

/// The `error` member of the one JSON object a failed command printed to standard error.
fn reported_error(stderr: &[u8]) -> Value {
    let diagnostic = String::from_utf8_lossy(stderr);
    let reported: Value =
        serde_json::from_str(diagnostic.trim_end()).expect("one JSON error object");
    reported["error"].clone()
}
