use clap::{Arg, ArgAction, Command};
use serde::Serialize;
use serde_json::Value;

use crate::application::response::{InspectResponse, LaunchResponse, ListResponse};
use crate::context::Context;
use crate::error::ErrorCode;
use crate::ui::request::{
    AccessMode, AccessProfile, DEFAULT_NATIVE_DEPTH, DEFAULT_NATIVE_LIMIT, FieldGroup,
    MAX_NATIVE_DEPTH, MAX_NATIVE_LIMIT, TraversalKind,
};
use crate::ui::response::{
    ConfirmedClose, CorrelateResponse, NativeFontsResponse, NativeNodeResponse,
    NativeNodesResponse, OpenResponse, PreflightResponse, PrepareHostResponse, ScreenshotResponse,
    TreeResponse, WindowsResponse,
};
use crate::ui::tree::MAX_DEPTH;

use super::decimal::Decimal;
use super::native;
use super::output_path::OutputPath;
use super::schema::{SchemaResponse, output_schema};

/// How one option or command depends on another, for combinations clap's declarative conflicts
/// cannot express because they depend on a supplied value.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Relation {
    /// The subject is accepted only together with every object.
    Requires,
    /// The subject is rejected together with any object.
    Forbids,
    /// At least one object is required.
    OneOf,
}

impl Relation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requires => "requires",
            Self::Forbids => "forbids",
            Self::OneOf => "one-of",
        }
    }
}

/// One usage rule the parser enforces and the registry publishes.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct UsageRule {
    /// The option, or the option and value, the rule is about; a command path when the rule is
    /// about the command itself.
    pub subject: &'static str,
    pub relation: Relation,
    pub objects: &'static [&'static str],
}

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
    pub usage: &'static [UsageRule],
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
        let ancestors = &specification.path[..specification.path.len() - 1];
        root = insert_leaf(root, ancestors, specification.parser());
    }
    root
}

/// Attach `leaf` under the chain of group names in `ancestors`, creating any intermediate group
/// that does not exist yet. A group requires one of its operations, so `recon ui` and `recon ui
/// ax` both report a missing operation rather than running.
fn insert_leaf(parent: Command, ancestors: &[&'static str], leaf: Command) -> Command {
    match ancestors.split_first() {
        None => parent.subcommand(leaf),
        Some((group, rest)) if parent.find_subcommand(group).is_some() => {
            parent.mut_subcommand(group, |command| insert_leaf(command, rest, leaf))
        }
        Some((group, rest)) => {
            parent.subcommand(insert_leaf(base(group).subcommand_required(true), rest, leaf))
        }
    }
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

fn pid() -> Arg {
    number::<1>("pid", "PID", "A process id greater than 0")
}

fn window() -> Arg {
    number::<0>("window", "ID", "A window id written as decimal digits")
}

/// `--session`, the explicit binding to one open inspection session.
///
/// Native commands require it; external commands accept it and keep their own observation
/// mechanism, so a session binds the instance rather than the way the instance is read.
fn session(required: bool) -> Arg {
    Arg::new("session")
        .long("session")
        .value_name("SESSION")
        .required(required)
        .value_parser(native::session_parser())
        .help("An open inspection session from recon ui session open, written as 32 lowercase hex digits")
}

fn root() -> Arg {
    Arg::new("root")
        .long("root")
        .value_name("HANDLE")
        .value_parser(native::handle_parser())
        .help("A live native handle from this session to start from; it must belong to --window when both are given")
}

fn kind() -> Arg {
    Arg::new("kind")
        .long("kind")
        .value_name("KIND")
        .default_value(TraversalKind::View.as_str())
        .value_parser(TraversalKind::ALL)
        .help("Which containment relation to walk; layer traversal needs --window or --root")
}

fn native_depth() -> Arg {
    Arg::new("depth")
        .long("depth")
        .value_name("N")
        .default_value(const_str(DEFAULT_NATIVE_DEPTH))
        .allow_hyphen_values(true)
        .value_parser(Decimal::<0, { MAX_NATIVE_DEPTH as u32 }>::parser())
        .help(format!(
            "A maximum traversal depth from 0 through {MAX_NATIVE_DEPTH} inclusive; a selected root is at depth 0"
        ))
}

fn limit() -> Arg {
    Arg::new("limit")
        .long("limit")
        .value_name("N")
        .default_value(const_str(DEFAULT_NATIVE_LIMIT))
        .allow_hyphen_values(true)
        .value_parser(Decimal::<1, { MAX_NATIVE_LIMIT as u32 }>::parser())
        .help(format!(
            "A maximum number of returned records from 1 through {MAX_NATIVE_LIMIT} inclusive"
        ))
}

fn fields() -> Arg {
    Arg::new("fields")
        .long("fields")
        .value_name("LIST")
        .default_value(FieldGroup::Basic.as_str())
        .value_parser(native::fields_parser())
        .help(format!(
            "A comma-separated, non-empty set of field groups without repeats: {}",
            FieldGroup::ALL.join(", ")
        ))
}

fn filter(name: &'static str, value_name: &'static str, description: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .value_name(value_name)
        .value_parser(native::filter_parser())
        .help(description)
}

/// Clap defaults are text; these are the only numeric defaults Recon publishes.
fn const_str(value: usize) -> &'static str {
    match value {
        DEFAULT_NATIVE_DEPTH => "3",
        DEFAULT_NATIVE_LIMIT => "200",
        _ => unreachable!("undeclared numeric default"),
    }
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
        usage: &[],
    }
}

/// The failures a command can report because it validated a supplied session, whether or not it
/// reads native objects afterwards.
const SESSION_BINDING: [ErrorCode; 5] = [
    ErrorCode::InspectionUnavailable,
    ErrorCode::InspectionProtocolMismatch,
    ErrorCode::InspectionFailed,
    ErrorCode::InspectionTimeout,
    ErrorCode::StaleSession,
];

/// The failures a native read can report in addition to validating its session.
const NATIVE_READ: [ErrorCode; 2] = [ErrorCode::InspectionBusy, ErrorCode::StaleNode];

/// Every `ui` operation runs inside the no-focus guard, so any of them can report an observed
/// disturbance of the user's foreground work, or that this Mac cannot watch one of the dimensions
/// the guard needs to make that judgement.
const NO_FOCUS_GUARD: [ErrorCode; 2] = [ErrorCode::UiInterference, ErrorCode::InterfaceUnavailable];

pub fn registry() -> Vec<CommandSpec> {
    use ErrorCode::*;
    let mut commands = vec![
        specification(
            &["app", "list"],
            "List discovered and running applications",
            "Read application bundles discovered through Spotlight and the running workspace without activating them.",
            vec![Arg::new("running")
                .long("running")
                .action(ArgAction::SetTrue)
                .help("List only applications with a running process; discovery still requires Spotlight")],
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
            "Read every window owned by one exact running instance without activating it. With a session, the window server inventory is accompanied by the instance's native windows and their relationships; the two inventories stay separate.",
            vec![target(), pid(), session(false)],
            output_schema::<WindowsResponse<'static>>,
        ),
        specification(
            &["ui", "ax", "tree"],
            "Read an application's accessibility tree",
            "Read the application or selected window's accessibility tree without activating it, with explicit truncation and failed-node markers. An optional session binds the instance, not the observation mechanism: this command never loads code or reads native objects.",
            vec![
                target(),
                pid(),
                window(),
                Arg::new("depth")
                    .long("depth")
                    .value_name("N")
                    .help(format!("A maximum tree depth from 0 through {MAX_DEPTH} inclusive; the root is at depth 0"))
                    .allow_hyphen_values(true)
                    .value_parser(Decimal::<0, { MAX_DEPTH as u32 }>::parser()),
                Arg::new("no-menu-bar")
                    .long("no-menu-bar")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("window")
                    .help("Exclude the application's menu bar, preserving status items and windows; a window tree contains no menu bar"),
                session(false),
            ],
            output_schema::<TreeResponse<'static>>,
        ),
        specification(
            &["ui", "screenshot"],
            "Capture one window to a new PNG file",
            "Capture the selected window, or the application's main window, without activating it. The output file must not exist. An optional session binds the instance; the capture still reads the same window server backing store.",
            vec![
                target(),
                pid(),
                window(),
                Arg::new("out")
                    .long("out")
                    .value_name("FILE")
                    .required(true)
                    .allow_hyphen_values(true)
                    .value_parser(OutputPath::parser())
                    .help("A path to a PNG file that does not exist yet and does not start with a hyphen"),
                session(false),
            ],
            output_schema::<ScreenshotResponse<'static>>,
        ),
        specification(
            &["ui", "session", "preflight"],
            "Report inspection access eligibility for a target",
            "Read target, host, and helper facts and report, for each access mode and profile, whether inspection access is eligible, blocked, or unknown, with reasons and evidence sources. It never loads code, launches, requests a privacy grant, or alters security, and an unreadable fact is unknown rather than false.",
            vec![target(), pid()],
            output_schema::<PreflightResponse<'static>>,
        ),
        specification(
            &["ui", "session", "prepare-host"],
            "Prepare this host for the research access profile",
            "Explicit, opt-in research-host preparation. With no flag it discloses the exact changes, asks for confirmation, sets the boot arguments Recon can set through sudo nvram, instructs a reboot, and prints the operator steps to disable System Integrity Protection in recovery, which Recon cannot perform itself. It never runs as a side effect of an inspection command.",
            vec![
                Arg::new("status")
                    .long("status")
                    .action(ArgAction::SetTrue)
                    .help("Report each prerequisite and what is missing, changing nothing"),
                Arg::new("revert")
                    .long("revert")
                    .action(ArgAction::SetTrue)
                    .conflicts_with("status")
                    .help("Remove the boot arguments Recon set and instruct a reboot"),
            ],
            output_schema::<PrepareHostResponse>,
        ),
        specification(
            &["ui", "session", "open"],
            "Open an inspection session on a target",
            "Establish one explicitly requested inspection session. existing connects only to a compatible helper already resident in the selected process, launch starts a non-running selected application with the helper enabled and refuses a running target, and attach loads the helper into a selected running process. There is no default mode and no fallback between modes.",
            vec![
                target(),
                Arg::new("mode")
                    .long("mode")
                    .value_name("MODE")
                    .required(true)
                    .value_parser(AccessMode::ALL)
                    .help("How to reach the target; there is no default and no fallback"),
                Arg::new("profile")
                    .long("profile")
                    .value_name("PROFILE")
                    .default_value(AccessProfile::Standard.as_str())
                    .value_parser(AccessProfile::ALL)
                    .help("Which access profile to run under; research requires an operator-prepared host"),
                pid(),
                Arg::new("allow-authorization")
                    .long("allow-authorization")
                    .action(ArgAction::SetTrue)
                    .help("Permit the disclosed macOS debugger-authorization interaction; accepted only with --mode attach"),
            ],
            output_schema::<OpenResponse>,
        ),
        specification(
            &["ui", "session", "close"],
            "Close one inspection session",
            "Revoke the session capability, end its inspection channel, and drop its handles. It leaves the target running, reports resident helper code separately, and fails with a typed error if teardown is unconfirmed.",
            vec![
                Arg::new("session")
                    .value_name("SESSION")
                    .required(true)
                    .value_parser(native::session_parser())
                    .help("The session to close, written as 32 lowercase hex digits"),
            ],
            output_schema::<ConfirmedClose>,
        ),
        specification(
            &["ui", "view", "tree"],
            "Read a bounded native view or layer subtree",
            "Walk one containment relation of the selected instance's native visual objects: views from a requested view, a window's content hierarchy, or every window's content hierarchy, or layers from an existing associated layer. It never creates a view, sets wantsLayer, or forces drawing or layout to obtain more data.",
            vec![
                target(),
                session(true),
                pid(),
                window(),
                root(),
                kind(),
                native_depth(),
                fields(),
                limit(),
            ],
            output_schema::<NativeNodesResponse>,
        ),
        specification(
            &["ui", "view", "node"],
            "Read one live native visual object",
            "Read one native window, view, layer, cell, or layout guide by its session handle and report its actual kind with the requested field groups. Relationships are typed references, not an implicit recursive expansion.",
            vec![
                target(),
                Arg::new("handle")
                    .value_name("HANDLE")
                    .required(true)
                    .value_parser(native::handle_parser())
                    .help("A live native handle from this session, written as 16 lowercase hex digits"),
                session(true),
                pid(),
                fields(),
            ],
            output_schema::<NativeNodeResponse>,
        ),
        specification(
            &["ui", "view", "find"],
            "Search a native scope with exact filters",
            "Search the selected live scope with exact filters, ANDed when combined; at least one filter is required. A limit stops the search after enough matches and identifies the unsearched scope rather than reporting a total it did not count.",
            vec![
                target(),
                session(true),
                pid(),
                window(),
                root(),
                kind(),
                filter("class", "NAME", "An exact runtime class name a node must have"),
                filter("identifier", "TEXT", "An exact native identifier a node must have"),
                native_depth(),
                fields(),
                limit(),
            ],
            output_schema::<NativeNodesResponse>,
        ),
        specification(
            &["ui", "view", "fonts"],
            "Inventory the fonts observed in a native scope",
            "Group the font descriptors observed on views, controls, cells, and bounded attributed-text runs in the selected scope, listing the objects that carry them. The inventory covers what the bounded walk observed; it is not every font the application can render.",
            vec![target(), session(true), pid(), window(), root(), native_depth(), limit()],
            output_schema::<NativeFontsResponse>,
        ),
        specification(
            &["ui", "correlate"],
            "Relate native and accessibility observations of one window",
            "Capture native and external accessibility evidence separately for the same verified process incarnation and selected window, then report evidence-backed candidate relationships. Candidates record the matching basis; a unique candidate is still not asserted to be the same object.",
            vec![target(), session(true), window().required(true), pid(), native_depth(), limit()],
            output_schema::<CorrelateResponse<'static>>,
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
        let targeted = command.arguments.iter().any(|argument| argument.get_id() == "target");
        if targeted || command.path == ["app", "list"] {
            command.errors.push(DiscoveryUnavailable);
        }
        if targeted {
            command.errors.extend([NotFound, NotAnApplication, AmbiguousTarget]);
        }
        if command.path[0] == "ui" {
            command.errors.extend(NO_FOCUS_GUARD);
            if targeted {
                command.errors.extend([AmbiguousInstance, NotRunning, IdentityUnavailable]);
            }
        }
        match command.path {
            ["app", "launch"] => {
                command.effects = &["activates-application"];
                command.errors.push(LaunchFailed);
            }
            ["ui", "windows"] => {
                command.permissions = &[("screen-recording", None)];
                command.errors.extend(SESSION_BINDING);
            }
            ["ui", "ax", "tree"] => {
                command.permissions = &[("accessibility", None)];
                command.errors.push(Timeout);
                command.errors.extend(SESSION_BINDING);
            }
            ["ui", "screenshot"] => {
                command.effects = &["writes-file"];
                command.permissions = &[
                    ("screen-recording", None),
                    ("accessibility", Some("when --window is absent")),
                ];
                command.errors.extend([Timeout, CaptureFailed, AlreadyExists, IoFailed]);
                command.errors.extend(SESSION_BINDING);
            }
            ["ui", "session", "preflight"] => {
                command.errors.push(InspectionUnavailable);
            }
            ["ui", "session", "prepare-host"] => {
                command.effects = &["changes-host-security", "requires-administrator"];
                command.errors.extend([InspectionUnavailable, IoFailed]);
            }
            ["ui", "session", "open"] => {
                command.effects = &[
                    "loads-code-into-target",
                    "launches-application",
                    "executes-target-code",
                    "may-request-debugger-authorization",
                ];
                command.errors.extend(SESSION_BINDING);
                command.errors.extend([InspectionBusy, LaunchFailed]);
                command.usage = &[
                    UsageRule {
                        subject: "--allow-authorization",
                        relation: Relation::Requires,
                        objects: &["--mode attach"],
                    },
                    UsageRule {
                        subject: "--pid",
                        relation: Relation::Forbids,
                        objects: &["--mode launch"],
                    },
                ];
            }
            ["ui", "session", "close"] => {
                command.errors.extend(SESSION_BINDING);
            }
            ["ui", "view", "node"] => {
                command.effects = &["executes-target-code"];
                command.errors.extend(SESSION_BINDING);
                command.errors.extend(NATIVE_READ);
            }
            ["ui", "view", "tree"] | ["ui", "view", "find"] | ["ui", "view", "fonts"] => {
                command.effects = &["executes-target-code"];
                command.permissions = &[("screen-recording", Some("when --window is supplied"))];
                command.errors.extend(SESSION_BINDING);
                command.errors.extend(NATIVE_READ);
                command.usage = if command.path == ["ui", "view", "find"] {
                    &[
                        UsageRule {
                            subject: "--kind layer",
                            relation: Relation::OneOf,
                            objects: &["--window", "--root"],
                        },
                        UsageRule {
                            subject: "ui view find",
                            relation: Relation::OneOf,
                            objects: &["--class", "--identifier"],
                        },
                    ]
                } else if command.path == ["ui", "view", "tree"] {
                    &[UsageRule {
                        subject: "--kind layer",
                        relation: Relation::OneOf,
                        objects: &["--window", "--root"],
                    }]
                } else {
                    &[]
                };
            }
            ["ui", "correlate"] => {
                command.effects = &["executes-target-code"];
                command.permissions = &[("screen-recording", None), ("accessibility", None)];
                command.errors.push(Timeout);
                command.errors.extend(SESSION_BINDING);
                command.errors.extend(NATIVE_READ);
            }
            _ => {}
        }
        if !command.permissions.is_empty() {
            command.errors.push(PermissionDenied);
        }
    }
    commands
}
