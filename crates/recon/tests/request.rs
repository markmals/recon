use recon::error::{ErrorCode, Failure};
use recon::request::{AppRequest, Request, parse_arguments};

fn parse(words: &[&str]) -> Result<Request, Failure> {
    let arguments: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    parse_arguments(&arguments)
}

fn rejects(words: &[&str]) {
    match parse(words) {
        Ok(request) => panic!("{words:?} should not parse, got {request:?}"),
        Err(error) => assert_eq!(error.code, ErrorCode::InvalidArgument),
    }
}

#[test]
fn app_list_without_options_keeps_the_full_inventory() {
    assert_eq!(parse(&["app", "list"]), Ok(Request::App(AppRequest::List { running: false })));
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
    rejects(&["app"]);
    rejects(&["app", "open", "Notes"]);
    rejects(&["app", "LIST"]);
    rejects(&["apps", "list"]);
    rejects(&["inspect", "Notes"]);
}
