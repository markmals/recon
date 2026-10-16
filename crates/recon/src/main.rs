use std::env;
#[cfg(target_os = "macos")]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use recon::application::Application;
use recon::ui::request::{USAGE, parse_ui_arguments};
#[cfg(target_os = "macos")]
use serde::Serialize;

#[cfg(target_os = "macos")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListedApplication<'a> {
    name: &'a str,
    bundle_id: Option<&'a str>,
    path: &'a PathBuf,
    running_pids: &'a [u32],
}

#[cfg(target_os = "macos")]
#[derive(Serialize)]
struct ListResponse<'a> {
    discovery: &'static str,
    applications: Vec<ListedApplication<'a>>,
}

#[cfg(target_os = "macos")]
#[derive(Serialize)]
struct InspectResponse<'a> {
    application: &'a Application,
}

#[cfg(target_os = "macos")]
#[derive(Serialize)]
struct LaunchResponse<'a> {
    path: &'a PathBuf,
    pid: u32,
}

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
    let arguments = arguments()?;
    let words: Vec<&str> = arguments.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["ui", ..] => run_ui(&arguments[1..]),
        ["app", "list"] => run_app("list", None),
        ["app", "inspect", target] => run_app("inspect", Some(target)),
        ["app", "launch", target] => run_app("launch", Some(target)),
        _ => Err(USAGE.to_owned()),
    }
}

fn arguments() -> Result<Vec<String>, String> {
    env::args_os()
        .skip(1)
        .map(|argument| argument.into_string().map_err(|_| "invalid UTF-8 argument".to_owned()))
        .collect()
}

fn run_ui(arguments: &[String]) -> Result<String, String> {
    let request = parse_ui_arguments(arguments)?;

    #[cfg(not(target_os = "macos"))]
    {
        let _ = request;
        Err("UI observation is supported only on macOS".to_owned())
    }
    #[cfg(target_os = "macos")]
    {
        recon::macos::ui::observe(&request)
    }
}

fn run_app(command: &str, target: Option<&str>) -> Result<String, String> {
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (command, target);
        Err("application investigation is supported only on macOS".to_owned())
    }
    #[cfg(target_os = "macos")]
    {
        run_app_macos(command, target)
    }
}

#[cfg(target_os = "macos")]
fn run_app_macos(command: &str, target: Option<&str>) -> Result<String, String> {
    use recon::macos;

    match command {
        "list" => {
            let applications = macos::applications()?;
            let entries = applications
                .iter()
                .map(|application| ListedApplication {
                    name: &application.name,
                    bundle_id: application.bundle_id.as_deref(),
                    path: &application.path,
                    running_pids: &application.running_pids,
                })
                .collect();
            serialize(&ListResponse { discovery: "indexed-and-running", applications: entries })
        }
        "inspect" => macos::with_target(target.unwrap(), |application| {
            serialize(&InspectResponse { application })
        }),
        "launch" => macos::with_target(target.unwrap(), |application| {
            if application.executable_path.is_none() {
                return Err(format!(
                    "cannot launch {}: application executable is missing",
                    application.path.display()
                ));
            }
            let pid = macos::launch(&application.path)?;
            serialize(&LaunchResponse { path: &application.path, pid })
        }),
        _ => unreachable!(),
    }
}

#[cfg(target_os = "macos")]
fn serialize(response: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(response).map_err(|error| format!("cannot encode response: {error}"))
}
