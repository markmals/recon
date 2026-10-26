//! The executable keeps successful output and the single structured error stream separate.

use recon::error::{ErrorCode, Failure};
use serde_json::json;
use std::env;

#[cfg(target_os = "macos")]
use recon::macos::{investigate, observe};
use recon::request::AppRequest;
use recon::request::{Request, parse_arguments};
use recon::ui::request::UiRequest;

fn main() {
    match run() {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("{}", serde_json::to_string(&error.report()).expect("error JSON serializes"));
            std::process::exit(error.exit_code());
        }
    }
}

fn run() -> Result<String, Failure> {
    let arguments = arguments()?;
    let request = parse_arguments(&arguments)?;
    let command = command_path(&request);
    let response = match request {
        Request::App(request) => investigate(&request),
        Request::Ui(request) => observe(&request),
        Request::Help(text) | Request::Version(text) => Ok(text.trim_end_matches('\n').to_owned()),
        Request::Schema(path) => recon::cli::schema::describe(&path).and_then(|response| {
            serde_json::to_string(&response)
                .map_err(|error| Failure::internal(format!("cannot encode response: {error}")))
        }),
        Request::Context => recon::context::context(),
    };
    response.map_err(|error| if command.is_empty() { error } else { error.with_command(command) })
}

fn arguments() -> Result<Vec<String>, Failure> {
    let mut first_invalid = None;
    let arguments: Vec<_> = env::args_os()
        .skip(1)
        .enumerate()
        .map(|(index, argument)| match argument.into_string() {
            Ok(argument) => argument,
            Err(argument) => {
                first_invalid.get_or_insert(index);
                argument.to_string_lossy().into_owned()
            }
        })
        .collect();
    if recon::cli::help_requested(&arguments) {
        return Ok(arguments);
    }
    let Some(index) = first_invalid else {
        return Ok(arguments);
    };
    let failure = Failure::new(
        ErrorCode::InvalidArgument,
        "invalid UTF-8 argument",
        json!({"argument":null,"value":arguments[index],"expected":"a UTF-8 argument","accepted":null}),
    );
    Err(match recon::cli::resolved_command(&arguments[..index]) {
        Some(command) => failure.with_command(command),
        None => failure,
    })
}

fn command_path(request: &Request) -> &'static [&'static str] {
    match request {
        Request::App(AppRequest::List) => &["app", "list"],
        Request::App(AppRequest::Inspect { .. }) => &["app", "inspect"],
        Request::App(AppRequest::Launch { .. }) => &["app", "launch"],
        Request::Ui(UiRequest::Windows { .. }) => &["ui", "windows"],
        Request::Ui(UiRequest::Tree { .. }) => &["ui", "tree"],
        Request::Ui(UiRequest::Screenshot { .. }) => &["ui", "screenshot"],
        Request::Schema(_) => &["schema"],
        Request::Context => &["context"],
        Request::Help(_) | Request::Version(_) => &[],
    }
}

#[cfg(not(target_os = "macos"))]
fn investigate(_request: &AppRequest) -> Result<String, Failure> {
    Err(unsupported("application investigation is supported only on macOS"))
}

#[cfg(not(target_os = "macos"))]
fn observe(_request: &UiRequest) -> Result<String, Failure> {
    Err(unsupported("UI observation is supported only on macOS"))
}

#[cfg(not(target_os = "macos"))]
fn unsupported(message: &str) -> Failure {
    Failure::new(
        ErrorCode::UnsupportedHost,
        message,
        json!({"host":env::consts::OS,"supportedHosts":["macos"]}),
    )
}
