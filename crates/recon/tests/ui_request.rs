use std::path::PathBuf;

use recon::application::Pid;
use recon::error::{ErrorCode, Failure};
use recon::request::{Request, parse_arguments};
use recon::ui::request::{Selection, UiRequest};
use recon::ui::window::WindowId;
mod support;

fn arguments(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

fn parse(words: &[&str]) -> Result<UiRequest, Failure> {
    let mut command = vec!["ui".to_owned()];
    command.extend(arguments(words));
    parse_arguments(&command).map(|request| match request {
        Request::Ui(request) => request,
        _ => panic!("expected UI request"),
    })
}

fn rejects(words: &[&str]) {
    match parse(words) {
        Ok(request) => panic!("{words:?} should not parse, got {request:?}"),
        Err(error) => {
            support::validate_failure(&error.report(), &[]);
            assert_eq!(error.code, ErrorCode::InvalidArgument);
        }
    }
}

#[test]
fn windows_takes_a_target_and_an_optional_pid() {
    assert_eq!(
        parse(&["windows", "Logic Pro"]).unwrap(),
        UiRequest::Windows { selection: Selection { target: "Logic Pro".to_owned(), pid: None } }
    );
    assert_eq!(
        parse(&["windows", "Logic Pro", "--pid", "412"]).unwrap(),
        UiRequest::Windows {
            selection: Selection { target: "Logic Pro".to_owned(), pid: Some(Pid(412)) },
        }
    );
}

#[test]
fn tree_takes_pid_and_window_flags_in_any_order() {
    let expected = UiRequest::Tree {
        selection: Selection { target: "/Applications/Notes.app".to_owned(), pid: Some(Pid(7)) },
        window: Some(WindowId(53510)),
    };
    assert_eq!(
        parse(&["tree", "/Applications/Notes.app", "--pid", "7", "--window", "53510"]).unwrap(),
        expected
    );
    assert_eq!(
        parse(&["tree", "/Applications/Notes.app", "--window", "53510", "--pid", "7"]).unwrap(),
        expected
    );
    assert_eq!(
        parse(&["tree", "Notes"]).unwrap(),
        UiRequest::Tree {
            selection: Selection { target: "Notes".to_owned(), pid: None },
            window: None,
        }
    );
}

#[test]
fn flags_may_come_before_the_target() {
    let safari = UiRequest::Tree {
        selection: Selection { target: "Safari".to_owned(), pid: Some(Pid(7)) },
        window: Some(WindowId(9)),
    };
    assert_eq!(parse(&["tree", "--pid", "7", "--window", "9", "Safari"]).unwrap(), safari);
    assert_eq!(parse(&["tree", "--window", "9", "Safari", "--pid", "7"]).unwrap(), safari);
    assert_eq!(parse(&["tree", "Safari", "--pid", "7", "--window", "9"]).unwrap(), safari);
    assert_eq!(
        parse(&["screenshot", "--out", "a.png", "Finder"]).unwrap(),
        UiRequest::Screenshot {
            selection: Selection { target: "Finder".to_owned(), pid: None },
            window: None,
            out: PathBuf::from("a.png"),
        }
    );
}

#[test]
fn screenshot_requires_an_output_file_and_accepts_a_window() {
    assert_eq!(
        parse(&["screenshot", "Notes", "--out", "shot.png"]).unwrap(),
        UiRequest::Screenshot {
            selection: Selection { target: "Notes".to_owned(), pid: None },
            window: None,
            out: PathBuf::from("shot.png"),
        }
    );
    assert_eq!(
        parse(&["screenshot", "Notes", "--window", "9", "--out", "/tmp/shot.png", "--pid", "3"])
            .unwrap(),
        UiRequest::Screenshot {
            selection: Selection { target: "Notes".to_owned(), pid: Some(Pid(3)) },
            window: Some(WindowId(9)),
            out: PathBuf::from("/tmp/shot.png"),
        }
    );
    rejects(&["screenshot", "Notes"]);
    rejects(&["screenshot", "Notes", "--window", "9"]);
}

#[test]
fn a_command_and_a_target_are_both_required() {
    rejects(&[]);
    rejects(&["windows"]);
    rejects(&["tree"]);
    rejects(&["screenshot", "--out", "shot.png"]);
    rejects(&["windows", "--pid", "412"]);
}

#[test]
fn an_unknown_command_is_rejected() {
    rejects(&["window", "Notes"]);
    rejects(&["WINDOWS", "Notes"]);
    rejects(&["capture", "Notes", "--out", "shot.png"]);
    rejects(&["--pid", "412"]);
}

#[test]
fn a_flag_that_does_not_apply_to_the_command_is_rejected() {
    rejects(&["windows", "Notes", "--window", "9"]);
    rejects(&["windows", "Notes", "--out", "shot.png"]);
    rejects(&["tree", "Notes", "--out", "shot.png"]);
    rejects(&["windows", "Notes", "--verbose"]);
    rejects(&["tree", "Notes", "--window-id", "9"]);
}

#[test]
fn a_repeated_flag_is_rejected() {
    rejects(&["windows", "Notes", "--pid", "1", "--pid", "1"]);
    rejects(&["tree", "Notes", "--window", "9", "--window", "10"]);
    rejects(&["screenshot", "Notes", "--out", "a.png", "--out", "b.png"]);
}

#[test]
fn a_flag_without_a_value_is_rejected() {
    rejects(&["windows", "Notes", "--pid"]);
    rejects(&["tree", "Notes", "--window"]);
    rejects(&["screenshot", "Notes", "--out"]);
    rejects(&["screenshot", "Notes", "--out", "--pid", "3"]);
    rejects(&["tree", "Notes", "--pid", "--window", "9"]);
}

#[test]
fn a_pid_must_be_a_positive_integer() {
    assert_eq!(
        parse(&["windows", "Notes", "--pid", "4294967295"]).unwrap(),
        UiRequest::Windows {
            selection: Selection { target: "Notes".to_owned(), pid: Some(Pid(u32::MAX)) },
        }
    );
    rejects(&["windows", "Notes", "--pid", "0"]);
    rejects(&["windows", "Notes", "--pid", "-1"]);
    rejects(&["windows", "Notes", "--pid", "4294967296"]);
    rejects(&["windows", "Notes", "--pid", "12.0"]);
    rejects(&["windows", "Notes", "--pid", "12x"]);
    rejects(&["windows", "Notes", "--pid", " 12"]);
    rejects(&["windows", "Notes", "--pid", ""]);
    rejects(&["windows", "Notes", "--pid", "+12"]);
}

#[test]
fn a_window_id_must_be_a_u32() {
    assert_eq!(
        parse(&["tree", "Notes", "--window", "0"]).unwrap(),
        UiRequest::Tree {
            selection: Selection { target: "Notes".to_owned(), pid: None },
            window: Some(WindowId(0)),
        }
    );
    rejects(&["tree", "Notes", "--window", "-3"]);
    rejects(&["tree", "Notes", "--window", "4294967296"]);
    rejects(&["tree", "Notes", "--window", "0x9"]);
}

#[test]
fn extra_positional_arguments_are_rejected() {
    rejects(&["windows", "Notes", "Mail"]);
    rejects(&["tree", "Notes", "--pid", "3", "Mail"]);
    rejects(&["screenshot", "Notes", "--out", "a.png", "b.png"]);
}
