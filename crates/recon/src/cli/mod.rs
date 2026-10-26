mod decimal;
mod help;
mod output_path;
mod parse_error;
pub mod registry;
pub mod schema;

use decimal::Decimal;
use output_path::OutputPath;
use std::ffi::OsStr;
use std::path::PathBuf;

use crate::application::Pid;
use crate::error::Failure;
use crate::request::{AppRequest, Request};
use crate::ui::request::{Selection, UiRequest};
use crate::ui::window::WindowId;

const HELP_FLAGS: [&str; 2] = ["--help", "-h"];
const VERSION_FLAG: &str = "--version";

pub fn help_requested(arguments: &[impl AsRef<OsStr>]) -> bool {
    arguments.is_empty()
        || arguments.first().is_some_and(|word| word.as_ref() == "help")
        || arguments.iter().any(|word| HELP_FLAGS.iter().any(|flag| word.as_ref() == *flag))
}

pub fn parse(arguments: &[String]) -> Result<Request, Failure> {
    let registry = registry::registry();
    let help_requested = help_requested(arguments);
    let mut words = arguments
        .iter()
        .skip(usize::from(arguments.first().is_some_and(|word| word == "help")))
        .map(String::as_str)
        .filter(|word| !HELP_FLAGS.contains(word));
    let path = resolved_path([words.next(), words.next()], &registry);
    if help_requested {
        return Ok(Request::Help(help::render(&registry, path)));
    }
    if arguments == [VERSION_FLAG] {
        return Ok(Request::Version(version()));
    }
    let entry = registry.iter().find(|entry| entry.path == path);
    let matches = registry::parser(&registry)
        .try_get_matches_from(std::iter::once("recon").chain(arguments.iter().map(String::as_str)))
        .map_err(|error| parse_error::convert(error, arguments, entry, path, &registry))?;
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
            "list" => AppRequest::List,
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
        "tree" => UiRequest::Tree { selection, window: window() },
        "screenshot" => UiRequest::Screenshot {
            selection,
            window: window(),
            out: PathBuf::from(&matches.get_one::<OutputPath>("out").expect("required output").0),
        },
        _ => unreachable!("registry operation"),
    }))
}

pub fn resolved_command(arguments: &[String]) -> Option<&'static [&'static str]> {
    let registry = registry::registry();
    let path = resolved_path(
        [arguments.first().map(String::as_str), arguments.get(1).map(String::as_str)],
        &registry,
    );
    registry.iter().find(|entry| entry.path == path).map(|entry| entry.path)
}

fn resolved_path(
    words: [Option<&str>; 2],
    registry: &[registry::CommandSpec],
) -> &'static [&'static str] {
    if let Some(entry) = registry.iter().find(|entry| {
        words.iter().take(entry.path.len()).copied().flatten().eq(entry.path.iter().copied())
    }) {
        return entry.path;
    }
    registry
        .iter()
        .find(|entry| entry.path.len() > 1 && entry.path.first().copied() == words[0])
        .map_or(&[], |entry| &entry.path[..1])
}

pub fn version() -> String {
    let base = concat!("recon ", env!("CARGO_PKG_VERSION"));
    match (option_env!("RECON_COMMIT_SHORT"), option_env!("RECON_COMMIT_DATE")) {
        (Some(commit), Some(date)) if !commit.is_empty() => format!("{base} ({commit} {date})"),
        _ => base.to_owned(),
    }
}
