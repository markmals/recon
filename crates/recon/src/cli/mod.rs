mod decimal;
mod help;
mod native;
mod output_path;
mod parse_error;
pub mod registry;
pub mod schema;

use decimal::Decimal;
use native::FilterText;
use output_path::OutputPath;
use std::ffi::OsStr;
use std::path::PathBuf;

use crate::application::Pid;
use crate::error::Failure;
use crate::inspection::SessionId;
use crate::request::{AppRequest, Request};
use crate::ui::request::{
    AccessMode, AccessProfile, FieldSet, Filters, HostPreparation, MAX_NATIVE_DEPTH,
    MAX_NATIVE_LIMIT, NativeScope, NodeHandle, Selection, SessionRequest, TraversalKind, UiRequest,
    ViewRequest,
};
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
            Ok(Request::Ui(ui_request(operation, path, leaf)?))
        }
        _ => unreachable!("registry command"),
    }
}

/// Build one `recon ui` request, enforcing the option combinations the registry publishes and
/// clap cannot express because they depend on a supplied value.
fn ui_request(
    operation: &[&str],
    path: &'static [&'static str],
    matches: &clap::ArgMatches,
) -> Result<UiRequest, Failure> {
    match operation {
        ["ui", "windows"] => {
            Ok(UiRequest::Windows { selection: selection(matches), session: session(matches) })
        }
        ["ui", "ax", "tree"] => Ok(UiRequest::AxTree {
            selection: selection(matches),
            session: session(matches),
            window: window(matches),
            depth: matches
                .get_one::<Decimal<0, { crate::ui::tree::MAX_DEPTH as u32 }>>("depth")
                .map(|number| number.0 as usize),
            no_menu_bar: matches.get_flag("no-menu-bar"),
        }),
        ["ui", "screenshot"] => Ok(UiRequest::Screenshot {
            selection: selection(matches),
            session: session(matches),
            window: window(matches),
            out: PathBuf::from(&matches.get_one::<OutputPath>("out").expect("required output").0),
        }),
        ["ui", "session", "preflight"] => {
            Ok(UiRequest::Session(SessionRequest::Preflight { selection: selection(matches) }))
        }
        ["ui", "session", "prepare-host"] => {
            let action = match (matches.get_flag("status"), matches.get_flag("revert")) {
                (true, _) => HostPreparation::Status,
                (_, true) => HostPreparation::Revert,
                _ => HostPreparation::Apply,
            };
            Ok(UiRequest::Session(SessionRequest::PrepareHost { action }))
        }
        ["ui", "session", "open"] => {
            let selection = selection(matches);
            let mode = choice(matches, "mode", AccessMode::parse);
            let profile = choice(matches, "profile", AccessProfile::parse);
            let allow_authorization = matches.get_flag("allow-authorization");
            if allow_authorization && mode != AccessMode::Attach {
                return Err(parse_error::usage(
                    path,
                    format!(
                        "--allow-authorization is accepted only with --mode attach, not --mode {mode}"
                    ),
                    Some("--allow-authorization"),
                    Some(mode.as_str()),
                    "--mode attach",
                    None,
                ));
            }
            if selection.pid.is_some() && mode == AccessMode::Launch {
                return Err(parse_error::usage(
                    path,
                    "--mode launch starts a non-running application, so it cannot select a running instance".to_owned(),
                    Some("--pid"),
                    Some(mode.as_str()),
                    "--mode existing or --mode attach",
                    None,
                ));
            }
            Ok(UiRequest::Session(SessionRequest::Open {
                selection,
                mode,
                profile,
                allow_authorization,
            }))
        }
        ["ui", "session", "close"] => Ok(UiRequest::Session(SessionRequest::Close {
            session: matches.get_one::<SessionId>("session").expect("required session").clone(),
        })),
        ["ui", "view", "tree"] => {
            let kind = choice(matches, "kind", TraversalKind::parse);
            let scope = scope(matches);
            layer_scope(path, kind, &scope)?;
            Ok(UiRequest::View(ViewRequest::Tree {
                selection: selection(matches),
                session: required_session(matches),
                scope,
                kind,
                depth: depth(matches),
                fields: fields(matches),
                limit: limit(matches),
            }))
        }
        ["ui", "view", "node"] => Ok(UiRequest::View(ViewRequest::Node {
            selection: selection(matches),
            session: required_session(matches),
            handle: matches.get_one::<NodeHandle>("handle").expect("required handle").clone(),
            fields: fields(matches),
        })),
        ["ui", "view", "find"] => {
            let kind = choice(matches, "kind", TraversalKind::parse);
            let scope = scope(matches);
            layer_scope(path, kind, &scope)?;
            let filters = Filters {
                class: matches.get_one::<FilterText>("class").map(|text| text.0.clone()),
                identifier: matches.get_one::<FilterText>("identifier").map(|text| text.0.clone()),
            };
            if filters.is_empty() {
                return Err(parse_error::usage(
                    path,
                    "recon ui view find searches with exact filters and requires at least one"
                        .to_owned(),
                    None,
                    None,
                    "--class or --identifier",
                    Some(vec!["--class".to_owned(), "--identifier".to_owned()]),
                ));
            }
            Ok(UiRequest::View(ViewRequest::Find {
                selection: selection(matches),
                session: required_session(matches),
                scope,
                kind,
                filters,
                depth: depth(matches),
                fields: fields(matches),
                limit: limit(matches),
            }))
        }
        ["ui", "view", "fonts"] => Ok(UiRequest::View(ViewRequest::Fonts {
            selection: selection(matches),
            session: required_session(matches),
            scope: scope(matches),
            depth: depth(matches),
            limit: limit(matches),
        })),
        ["ui", "correlate"] => Ok(UiRequest::Correlate {
            selection: selection(matches),
            session: required_session(matches),
            window: window(matches).expect("required window"),
            depth: depth(matches),
            limit: limit(matches),
        }),
        _ => unreachable!("registry ui operation"),
    }
}

fn selection(matches: &clap::ArgMatches) -> Selection {
    Selection {
        target: ui_target(matches),
        pid: matches.get_one::<Decimal<1>>("pid").map(|number| Pid(number.0)),
    }
}

fn window(matches: &clap::ArgMatches) -> Option<WindowId> {
    matches.get_one::<Decimal<0>>("window").map(|number| WindowId(number.0))
}

fn session(matches: &clap::ArgMatches) -> Option<SessionId> {
    matches.get_one::<SessionId>("session").cloned()
}

fn required_session(matches: &clap::ArgMatches) -> SessionId {
    session(matches).expect("required session")
}

fn scope(matches: &clap::ArgMatches) -> NativeScope {
    NativeScope { window: window(matches), root: matches.get_one::<NodeHandle>("root").cloned() }
}

fn depth(matches: &clap::ArgMatches) -> usize {
    matches
        .get_one::<Decimal<0, { MAX_NATIVE_DEPTH as u32 }>>("depth")
        .expect("depth has a default")
        .0 as usize
}

fn limit(matches: &clap::ArgMatches) -> usize {
    matches
        .get_one::<Decimal<1, { MAX_NATIVE_LIMIT as u32 }>>("limit")
        .expect("limit has a default")
        .0 as usize
}

fn fields(matches: &clap::ArgMatches) -> FieldSet {
    matches.get_one::<FieldSet>("fields").expect("fields has a default").clone()
}

/// Read an option whose accepted values clap already restricted to the ones `parse` knows.
fn choice<T>(matches: &clap::ArgMatches, name: &str, parse: fn(&str) -> Option<T>) -> T {
    let value = matches.get_one::<String>(name).expect("a required or defaulted choice");
    parse(value).expect("clap accepted an unknown value")
}

/// Layer traversal reads an existing associated layer, so it needs a native window, view, or
/// layer to start from. Omitting that scope is a usage error, not an empty successful result.
fn layer_scope(
    path: &'static [&'static str],
    kind: TraversalKind,
    scope: &NativeScope,
) -> Result<(), Failure> {
    if kind == TraversalKind::Layer && scope.window.is_none() && scope.root.is_none() {
        return Err(parse_error::usage(
            path,
            "layer traversal needs an existing layer scope: a native window, view, or layer"
                .to_owned(),
            Some("--kind"),
            Some(kind.as_str()),
            "--kind layer with --window or --root",
            Some(vec!["--window".to_owned(), "--root".to_owned()]),
        ));
    }
    Ok(())
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
