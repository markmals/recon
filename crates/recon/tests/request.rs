use recon::request::{AppRequest, Request, USAGE, parse_arguments};

fn parse(words: &[&str]) -> Result<Request, String> {
    let arguments: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    parse_arguments(&arguments)
}

fn rejects(words: &[&str]) {
    match parse(words) {
        Ok(request) => panic!("{words:?} should not parse, got {request:?}"),
        Err(error) => assert_eq!(error, USAGE, "{words:?} should report the usage text"),
    }
}

#[test]
fn app_list_takes_no_arguments() {
    assert_eq!(parse(&["app", "list"]), Ok(Request::App(AppRequest::List)));
    rejects(&["app", "list", "Notes"]);
}

#[test]
fn inspect_and_launch_take_exactly_one_target() {
    assert_eq!(
        parse(&["app", "inspect", "Logic Pro"]),
        Ok(Request::App(AppRequest::Inspect { target: "Logic Pro".to_owned() }))
    );
    assert_eq!(
        parse(&["app", "launch", "/Applications/Notes.app"]),
        Ok(Request::App(AppRequest::Launch { target: "/Applications/Notes.app".to_owned() }))
    );
    rejects(&["app", "inspect"]);
    rejects(&["app", "launch"]);
    rejects(&["app", "inspect", "Notes", "Mail"]);
    rejects(&["app", "launch", "Notes", "Mail"]);
}

#[test]
fn an_unknown_group_or_app_command_is_rejected() {
    rejects(&[]);
    rejects(&["app"]);
    rejects(&["app", "open", "Notes"]);
    rejects(&["app", "LIST"]);
    rejects(&["apps", "list"]);
    rejects(&["inspect", "Notes"]);
}

#[test]
fn the_usage_text_covers_both_command_groups() {
    assert!(USAGE.contains("recon app list | inspect <target> | launch <target>"), "{USAGE}");
    assert!(USAGE.contains("recon ui windows <target> [--pid <pid>]"), "{USAGE}");
    assert!(USAGE.contains("recon ui tree <target> [--pid <pid>] [--window <id>]"), "{USAGE}");
    assert!(
        USAGE.contains("recon ui screenshot <target> [--pid <pid>] [--window <id>] --out <file>"),
        "{USAGE}"
    );
}
