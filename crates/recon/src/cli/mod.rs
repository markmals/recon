mod decimal;
mod help;
mod parse_error;
pub mod registry;
pub mod schema;

use decimal::Decimal;
use std::ffi::OsStr;
use std::path::PathBuf;

use crate::application::Pid;
use crate::error::Failure;
use crate::request::{AppRequest, Request};
use crate::ui::request::{Selection, UiRequest};
use crate::ui::window::WindowId;

pub fn help_requested(arguments: &[impl AsRef<OsStr>]) -> bool {
    arguments.is_empty()
        || arguments.first().is_some_and(|word| word.as_ref() == "help")
        || arguments.iter().any(|word| word.as_ref() == "--help" || word.as_ref() == "-h")
}

pub fn parse(arguments: &[String]) -> Result<Request, Failure> {
    let registry = registry::registry();
    let help_requested = help_requested(arguments);
    let words = arguments
        .iter()
        .map(String::as_str)
        .filter(|word| !matches!(*word, "help" | "--help" | "-h"))
        .collect::<Vec<_>>();
    let path = resolved_path(&words, &registry);
    if help_requested {
        return Ok(Request::Help(help::render(&registry, &path)));
    }
    if arguments == ["--version"] {
        return Ok(Request::Version(version()));
    }
    let entry = registry.iter().find(|entry| entry.path == path);
    let matches = registry::parser(&registry)
        .try_get_matches_from(std::iter::once("recon").chain(arguments.iter().map(String::as_str)))
        .map_err(|error| parse_error::convert(error, arguments, entry, &path, &registry))?;
    let (group, group_matches) = matches.subcommand().expect("nonempty validated command");
    if group == "context" {
        return Ok(Request::Context);
    }
    if group == "schema" {
        return Ok(Request::Schema(
            group_matches
                .get_many::<String>("command")
                .map(|words| words.cloned().collect())
                .unwrap_or_default(),
        ));
    }
    let (operation, matches) = group_matches.subcommand().expect("validated operation");
    let target = || matches.get_one::<String>("target").expect("required target").clone();
    if group == "app" {
        return Ok(Request::App(match operation {
            "list" => AppRequest::List { running: matches.get_flag("running") },
            "inspect" => AppRequest::Inspect { target: target() },
            "launch" => AppRequest::Launch { target: target() },
            _ => unreachable!("registry operation"),
        }));
    }
    let selection = Selection {
        target: target(),
        pid: matches.get_one::<Decimal<1>>("pid").map(|number| Pid(number.0)),
    };
    let window = || matches.get_one::<Decimal<0>>("window").map(|number| WindowId(number.0));
    Ok(Request::Ui(match operation {
        "windows" => UiRequest::Windows { selection },
        "tree" => UiRequest::Tree {
            selection,
            window: window(),
            depth: matches
                .get_one::<Decimal<0, { crate::ui::tree::MAX_DEPTH as u32 }>>("depth")
                .map(|number| number.0 as usize),
            no_menu_bar: matches.get_flag("no-menu-bar"),
        },
        "screenshot" => UiRequest::Screenshot {
            selection,
            window: window(),
            out: PathBuf::from(matches.get_one::<String>("out").expect("required output")),
        },
        _ => unreachable!("registry operation"),
    }))
}

fn resolved_path<'a>(words: &[&str], registry: &'a [registry::CommandSpec]) -> Vec<&'a str> {
    if let Some(entry) = registry.iter().find(|entry| words.starts_with(entry.path)) {
        return entry.path.to_vec();
    }
    match words.first() {
        Some(&"app") => vec!["app"],
        Some(&"ui") => vec!["ui"],
        _ => Vec::new(),
    }
}

pub fn version() -> String {
    let base = concat!("recon ", env!("CARGO_PKG_VERSION"));
    match (option_env!("RECON_COMMIT_SHORT"), option_env!("RECON_COMMIT_DATE")) {
        (Some(commit), Some(date)) if !commit.is_empty() => format!("{base} ({commit} {date})"),
        _ => base.to_owned(),
    }
}
