use std::env;

#[cfg(target_os = "macos")]
use recon::macos::{investigate, observe};
#[cfg(not(target_os = "macos"))]
use recon::request::AppRequest;
use recon::request::{Request, parse_arguments};
#[cfg(not(target_os = "macos"))]
use recon::ui::request::UiRequest;

fn main() {
    match run() {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("recon: {error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<String, String> {
    match parse_arguments(&arguments()?)? {
        Request::App(request) => investigate(&request),
        Request::Ui(request) => observe(&request),
    }
}

fn arguments() -> Result<Vec<String>, String> {
    env::args_os()
        .skip(1)
        .map(|argument| argument.into_string().map_err(|_| "invalid UTF-8 argument".to_owned()))
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn investigate(_request: &AppRequest) -> Result<String, String> {
    Err("application investigation is supported only on macOS".to_owned())
}

#[cfg(not(target_os = "macos"))]
fn observe(_request: &UiRequest) -> Result<String, String> {
    Err("UI observation is supported only on macOS".to_owned())
}
