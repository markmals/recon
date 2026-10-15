use std::env;
#[cfg(target_os = "macos")]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use recon::application::Application;
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
    let mut args = env::args_os();
    let _program = args.next();
    let parse = |argument: Option<std::ffi::OsString>| {
        argument
            .map(|argument| argument.into_string().map_err(|_| "invalid UTF-8 argument".to_owned()))
            .transpose()
    };
    let group = parse(args.next())?;
    let command = parse(args.next())?;
    let target = parse(args.next())?;
    let syntax =
        match (group.as_deref(), command.as_deref(), target.as_deref(), parse(args.next())?) {
            (Some("app"), Some("list"), None, None) => "list",
            (Some("app"), Some("inspect"), Some(_), None) => "inspect",
            (Some("app"), Some("launch"), Some(_), None) => "launch",
            _ => {
                return Err("usage: recon app list | inspect <target> | launch <target>".to_owned());
            }
        };

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (syntax, target);
        Err("application investigation is supported only on macOS".to_owned())
    }
    #[cfg(target_os = "macos")]
    {
        run_macos(syntax, target.as_deref())
    }
}

#[cfg(target_os = "macos")]
fn run_macos(command: &str, target: Option<&str>) -> Result<String, String> {
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
