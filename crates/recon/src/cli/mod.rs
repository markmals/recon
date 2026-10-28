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
    let lead: Vec<&str> = words.by_ref().take(3).collect();
    let path = resolved_path(&lead, &registry);
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
    let mut resolved: Vec<&str> = Vec::new();
    let mut leaf = &matches;
    while let Some((name, sub)) = leaf.subcommand() {
        resolved.push(name);
        leaf = sub;
    }
    match resolved.as_slice() {
        ["context"] => Ok(Request::Context),
        ["schema"] => Ok(Request::Schema(
            leaf.get_many::<String>("command")
                .map(|words| words.cloned().collect())
                .unwrap_or_default(),
        )),
        ["app", operation] => Ok(Request::App(match *operation {
            "list" => AppRequest::List { running: leaf.get_flag("running") },
            "inspect" => AppRequest::Inspect { target: ui_target(leaf) },
            "launch" => AppRequest::Launch { target: ui_target(leaf) },
            _ => unreachable!("registry app operation"),
        })),
        operation if operation.first() == Some(&"ui") => {
            let selection = Selection {
                target: ui_target(leaf),
                pid: leaf.get_one::<Decimal<1>>("pid").map(|number| Pid(number.0)),
            };
            let window = || leaf.get_one::<Decimal<0>>("window").map(|number| WindowId(number.0));
            Ok(Request::Ui(match operation {
                ["ui", "windows"] => UiRequest::Windows { selection },
                ["ui", "ax", "tree"] => UiRequest::AxTree {
                    selection,
                    window: window(),
                    depth: leaf
                        .get_one::<Decimal<0, { crate::ui::tree::MAX_DEPTH as u32 }>>("depth")
                        .map(|number| number.0 as usize),
                    no_menu_bar: leaf.get_flag("no-menu-bar"),
                },
                ["ui", "screenshot"] => UiRequest::Screenshot {
                    selection,
                    window: window(),
                    out: PathBuf::from(
                        &leaf.get_one::<OutputPath>("out").expect("required output").0,
                    ),
                },
                _ => unreachable!("registry ui operation"),
            }))
        }
        _ => unreachable!("registry command"),
    }
}

fn ui_target(matches: &clap::ArgMatches) -> String {
    matches.get_one::<String>("target").expect("required target").clone()
}

pub fn resolved_command(arguments: &[String]) -> Option<&'static [&'static str]> {
    let registry = registry::registry();
    let lead: Vec<&str> = arguments.iter().take(3).map(String::as_str).collect();
    let path = resolved_path(&lead, &registry);
    registry.iter().find(|entry| entry.path == path).map(|entry| entry.path)
}

/// Resolve the leading words to the deepest command path they name: a full command when they
/// reach a leaf, otherwise the deepest group prefix so help and errors can describe it.
fn resolved_path(words: &[&str], registry: &[registry::CommandSpec]) -> &'static [&'static str] {
    if let Some(entry) = registry.iter().find(|entry| {
        entry.path.len() <= words.len()
            && entry.path.iter().zip(words).all(|(expected, actual)| expected == actual)
    }) {
        return entry.path;
    }
    let mut deepest: &'static [&'static str] = &[];
    for entry in registry {
        let shared = entry
            .path
            .iter()
            .zip(words)
            .take_while(|(expected, actual)| expected == actual)
            .count();
        if shared > deepest.len() && shared < entry.path.len() {
            deepest = &entry.path[..shared];
        }
    }
    deepest
}

pub fn version() -> String {
    let base = concat!("recon ", env!("CARGO_PKG_VERSION"));
    match (option_env!("RECON_COMMIT_SHORT"), option_env!("RECON_COMMIT_DATE")) {
        (Some(commit), Some(date)) if !commit.is_empty() && !date.is_empty() => {
            format!("{base} ({commit} {date})")
        }
        _ => base.to_owned(),
    }
}
