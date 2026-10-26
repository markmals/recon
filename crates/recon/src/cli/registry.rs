use clap::{Arg, Command};
use serde_json::Value;

use crate::application::response::{InspectResponse, LaunchResponse, ListResponse};
use crate::context::Context;
use crate::error::ErrorCode;
use crate::ui::response::{ScreenshotResponse, TreeResponse, WindowsResponse};

use super::decimal::Decimal;
use super::output_path::OutputPath;
use super::schema::{SchemaResponse, output_schema};

pub struct CommandSpec {
    pub path: &'static [&'static str],
    pub summary: &'static str,
    pub description: &'static str,
    pub arguments: Vec<Arg>,
    pub effects: &'static [&'static str],
    pub permissions: &'static [(&'static str, Option<&'static str>)],
    pub hosts: &'static [&'static str],
    pub output: fn() -> Value,
    pub errors: Vec<ErrorCode>,
}

impl CommandSpec {
    pub fn parser(&self) -> Command {
        base(self.path.last().expect("command path"))
            .about(self.summary)
            .long_about(self.description)
            .args(self.arguments.iter().cloned())
    }
}

pub fn base(name: &'static str) -> Command {
    Command::new(name)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true)
        .color(clap::ColorChoice::Never)
        .styles(clap::builder::Styles::plain())
}

pub fn parser(registry: &[CommandSpec]) -> Command {
    let mut root = base("recon").subcommand_required(true);
    for specification in registry {
        let group = specification.path[0];
        if specification.path.len() == 1 {
            root = root.subcommand(specification.parser());
        } else if root.find_subcommand(group).is_some() {
            root = root.mut_subcommand(group, |command| command.subcommand(specification.parser()));
        } else {
            root = root.subcommand(
                base(group).subcommand_required(true).subcommand(specification.parser()),
            );
        }
    }
    root
}

fn target() -> Arg {
    Arg::new("target").value_name("TARGET").required(true)
        .help("An exact application name, case-insensitive bundle identifier, or absolute application bundle path")
}

fn number<const MINIMUM: u32>(
    name: &'static str,
    value_name: &'static str,
    description: &'static str,
) -> Arg {
    Arg::new(name)
        .long(name)
        .value_name(value_name)
        .help(description)
        .allow_hyphen_values(true)
        .value_parser(Decimal::<MINIMUM>::parser())
}

fn specification(
    path: &'static [&'static str],
    summary: &'static str,
    description: &'static str,
    arguments: Vec<Arg>,
    output: fn() -> Value,
) -> CommandSpec {
    CommandSpec {
        path,
        summary,
        description,
        arguments,
        effects: &[],
        permissions: &[],
        hosts: &["macos"],
        output,
        errors: vec![ErrorCode::InvalidArgument, ErrorCode::UnsupportedHost, ErrorCode::Internal],
    }
}

pub fn registry() -> Vec<CommandSpec> {
    use ErrorCode::*;
    let mut commands = vec![
        specification(
            &["app", "list"],
            "List discovered and running applications",
            "Read application bundles discovered through Spotlight and the running workspace without activating them.",
            vec![],
            output_schema::<ListResponse<'static>>,
        ),
        specification(
            &["app", "inspect"],
            "Inspect an application bundle",
            "Read bundle metadata and running process IDs without launching or activating the application.",
            vec![target()],
            output_schema::<InspectResponse<'static>>,
        ),
        specification(
            &["app", "launch"],
            "Launch or activate an application",
            "Explicitly open the exact selected bundle through macOS; the application may become foreground.",
            vec![target()],
            output_schema::<LaunchResponse<'static>>,
        ),
        specification(
            &["ui", "windows"],
            "List windows of a running application",
            "Read every window owned by one exact running instance without activating it.",
            vec![target(), number::<1>("pid", "PID", "A process id greater than 0")],
            output_schema::<WindowsResponse<'static>>,
        ),
        specification(
            &["ui", "tree"],
            "Read an application's accessibility tree",
            "Read the application or selected window's accessibility tree without activating it, with explicit truncation and failed-node markers.",
            vec![
                target(),
                number::<1>("pid", "PID", "A process id greater than 0"),
                number::<0>("window", "ID", "A window id written as decimal digits"),
            ],
            output_schema::<TreeResponse<'static>>,
        ),
        specification(
            &["ui", "screenshot"],
            "Capture one window to a new PNG file",
            "Capture the selected window, or the application's main window, without activating it. The output file must not exist.",
            vec![
                target(),
                number::<1>("pid", "PID", "A process id greater than 0"),
                number::<0>("window", "ID", "A window id written as decimal digits"),
                Arg::new("out")
                    .long("out")
                    .value_name("FILE")
                    .required(true)
                    .allow_hyphen_values(true)
                    .value_parser(OutputPath::parser())
                    .help("A path to a PNG file that does not exist yet and does not start with a hyphen"),
            ],
            output_schema::<ScreenshotResponse<'static>>,
        ),
        specification(
            &["schema"],
            "Describe commands and the error contract",
            "Read Recon's registry without consulting the host or filesystem. With a command path, return its full specification; otherwise return the index.",
            vec![
                Arg::new("command")
                    .value_name("COMMAND")
                    .num_args(0..)
                    .help("A registered command path"),
            ],
            output_schema::<SchemaResponse>,
        ),
        specification(
            &["context"],
            "Report Recon's identity and host capabilities",
            "Report this executable's identity, host, permissions and private interfaces. Re-executes to establish Recon's own privacy identity, but only preflights permissions and never requests them. Pre-spawn identity failures make grants unknown; a post-spawn wait failure is fatal.",
            vec![],
            output_schema::<Context>,
        ),
    ];
    for command in &mut commands {
        if command.path.len() == 1 {
            command.hosts = &["any"];
            command.errors = vec![InvalidArgument, Internal];
            if command.path == ["context"] {
                command.errors.push(IdentityUnavailable);
            }
            continue;
        }
        command.errors.push(DiscoveryUnavailable);
        if command.path != ["app", "list"] {
            command.errors.extend([NotFound, NotAnApplication, AmbiguousTarget]);
        }
        if command.path[0] == "ui" {
            command.errors.extend([
                AmbiguousInstance,
                NotRunning,
                PermissionDenied,
                IdentityUnavailable,
            ]);
            command.permissions = &[("screen-recording", None)];
        }
        match command.path {
            ["app", "launch"] => {
                command.effects = &["activates-application"];
                command.errors.push(LaunchFailed);
            }
            ["ui", "tree"] => {
                command.permissions = &[("accessibility", None)];
                command.errors.extend([Timeout, InterfaceUnavailable]);
            }
            ["ui", "screenshot"] => {
                command.effects = &["writes-file"];
                command.permissions = &[
                    ("screen-recording", None),
                    ("accessibility", Some("when --window is absent")),
                ];
                command.errors.extend([
                    Timeout,
                    CaptureFailed,
                    AlreadyExists,
                    IoFailed,
                    InterfaceUnavailable,
                ]);
            }
            _ => {}
        }
    }
    commands
}
