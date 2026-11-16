use recon::request::parse_arguments;
use recon_core::error::ErrorCode;

fn parse(words: &[&str]) -> Result<(), ErrorCode> {
    let arguments =
        std::iter::once("ui").chain(words.iter().copied()).map(str::to_owned).collect::<Vec<_>>();
    parse_arguments(&arguments).map(|_| ()).map_err(|error| error.code)
}

fn accepts(words: &[&str]) {
    assert_eq!(parse(words), Ok(()), "{words:?}");
}

fn rejects(words: &[&str]) {
    assert_eq!(parse(words), Err(ErrorCode::InvalidArgument), "{words:?}");
}

#[test]
fn proposal0007_click_requires_one_window_and_one_location_form() {
    accepts(&["click", "Notes", "--window", "123", "--role", "AXButton"]);
    accepts(&["click", "Notes", "--window", "123", "--title", "Share"]);
    accepts(&["click", "Notes", "--window", "123", "--identifier", "share"]);
    accepts(&["click", "Notes", "--window", "123", "--x", "160", "--y", "84"]);

    rejects(&["click", "Notes", "--role", "AXButton"]);
    rejects(&["click", "Notes", "--window", "123"]);
    rejects(&["click", "Notes", "--window", "123", "--x", "160"]);
    rejects(&["click", "Notes", "--window", "123", "--y", "84"]);
    rejects(&[
        "click", "Notes", "--window", "123", "--role", "AXButton", "--x", "160", "--y", "84",
    ]);
}

#[test]
fn proposal0007_click_rejects_values_outside_its_gesture_contract() {
    for method in ["auto", "ax", "mouse"] {
        accepts(&["click", "Notes", "--window", "123", "--title", "Share", "--method", method]);
    }
    for button in ["left", "right", "middle"] {
        accepts(&["click", "Notes", "--window", "123", "--x", "1", "--y", "2", "--button", button]);
    }
    accepts(&[
        "click",
        "Notes",
        "--window",
        "123",
        "--x",
        "1",
        "--y",
        "2",
        "--count",
        "2",
        "--modifiers",
        "cmd,ctrl,alt,shift",
    ]);

    rejects(&["click", "Notes", "--window", "123", "--title", "Share", "--method", "AX"]);
    rejects(&["click", "Notes", "--window", "123", "--x", "1", "--y", "2", "--button", "back"]);
    rejects(&["click", "Notes", "--window", "123", "--x", "1", "--y", "2", "--count", "0"]);
    rejects(&["click", "Notes", "--window", "123", "--x", "1", "--y", "2", "--count", "3"]);
    rejects(&[
        "click",
        "Notes",
        "--window",
        "123",
        "--x",
        "1",
        "--y",
        "2",
        "--modifiers",
        "cmd,cmd",
    ]);
    rejects(&[
        "click",
        "Notes",
        "--window",
        "123",
        "--x",
        "1",
        "--y",
        "2",
        "--modifiers",
        "cmd,command",
    ]);
    rejects(&[
        "click",
        "Notes",
        "--window",
        "123",
        "--x",
        "1",
        "--y",
        "2",
        "--modifiers",
        "cmd,shift,",
    ]);
    rejects(&["click", "Notes", "--window", "123", "--x", "NaN", "--y", "2"]);
    rejects(&["click", "Notes", "--window", "123", "--x", "1", "--y", "inf"]);
}

#[test]
fn proposal0007_forced_ax_rejects_mouse_only_gestures() {
    for arguments in [
        vec!["--method", "ax", "--x", "1", "--y", "2"],
        vec!["--method", "ax", "--title", "Share", "--button", "right"],
        vec!["--method", "ax", "--title", "Share", "--button", "middle"],
        vec!["--method", "ax", "--title", "Share", "--count", "2"],
        vec!["--method", "ax", "--title", "Share", "--modifiers", "shift"],
    ] {
        let mut words = vec!["click", "Notes", "--window", "123"];
        words.extend(arguments);
        rejects(&words);
    }
}

#[test]
fn proposal0007_scroll_requires_a_bounded_nonzero_delta_and_location() {
    accepts(&["scroll", "Notes", "--window", "123", "--x", "1", "--y", "2", "--dy", "300"]);
    accepts(&[
        "scroll", "Notes", "--window", "123", "--title", "Content", "--dx", "-10000", "--dy",
        "10000", "--unit", "pixel",
    ]);
    accepts(&[
        "scroll", "Notes", "--window", "123", "--x", "1", "--y", "2", "--dx", "-1000", "--dy",
        "1000", "--unit", "line",
    ]);

    rejects(&["scroll", "Notes", "--window", "123", "--x", "1", "--y", "2"]);
    rejects(&[
        "scroll", "Notes", "--window", "123", "--x", "1", "--y", "2", "--dx", "0", "--dy", "0",
    ]);
    rejects(&["scroll", "Notes", "--window", "123", "--x", "1", "--y", "2", "--dy", "10001"]);
    rejects(&["scroll", "Notes", "--window", "123", "--x", "1", "--y", "2", "--dy", "-10001"]);
    rejects(&[
        "scroll", "Notes", "--window", "123", "--x", "1", "--y", "2", "--dy", "1001", "--unit",
        "line",
    ]);
    rejects(&["scroll", "Notes", "--window", "123", "--x", "1", "--y", "2", "--dy", "1.5"]);
    rejects(&[
        "scroll", "Notes", "--window", "123", "--x", "1", "--y", "2", "--dy", "1", "--unit", "page",
    ]);
}

#[test]
fn proposal0007_key_accepts_the_complete_chord_grammar() {
    for chord in [
        "a",
        "Z",
        "0",
        "`",
        "-",
        "=",
        "[",
        "]",
        "\\",
        ";",
        "'",
        ",",
        ".",
        "/",
        "return",
        "tab",
        "space",
        "escape",
        "esc",
        "delete",
        "forward-delete",
        "left",
        "right",
        "up",
        "down",
        "home",
        "end",
        "pageup",
        "pagedown",
        "f1",
        "f12",
        "CMD+CTRL+OPT+SHIFT+B",
        "command+control+option+shift+b",
    ] {
        accepts(&["key", "Notes", chord, "--window", "123"]);
    }
}

#[test]
fn proposal0007_key_rejects_malformed_or_ambiguous_chords_before_input() {
    for chord in [
        "",
        "cmd",
        "cmd+shift",
        "a+b",
        "cmd+cmd+b",
        "cmd+command+b",
        "alt+option+b",
        "f0",
        "f13",
        "cmd++b",
        "+b",
        "cmd+b+",
        "cmd +b",
        "cmd+ b",
        "cmd\n+b",
        "?",
        "~",
        "🙂",
    ] {
        rejects(&["key", "Notes", chord, "--window", "123"]);
    }
    rejects(&["key", "Notes", "cmd+b"]);
    rejects(&["key", "Notes", "--window", "123"]);
}
