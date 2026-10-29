//! The `recon ui` command grammar.
//!
//! `ui` is the interface domain: `ax` queries the accessibility representation, `view` the native
//! visual one, `session` the inspection-access relationship, and `windows`/`screenshot`/`correlate`
//! are shared operations. A session is never implied: an external command runs exactly as it did
//! before when `--session` is absent, and a native command cannot run without one.

use std::fmt;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::application::Pid;
use crate::inspection::SessionId;
use crate::ui::window::WindowId;

/// The deepest native traversal a request may ask for. A selected root is depth zero.
pub const MAX_NATIVE_DEPTH: usize = 64;
/// The traversal depth a native request uses when `--depth` is absent.
pub const DEFAULT_NATIVE_DEPTH: usize = 3;
/// The largest number of records a native request may ask for, which is also the hard ceiling on
/// objects and relationships one request may visit in the target.
pub const MAX_NATIVE_LIMIT: usize = 25_000;
/// The number of records a native request returns when `--limit` is absent.
pub const DEFAULT_NATIVE_LIMIT: usize = 200;
/// Digits in a native handle's published spelling.
pub const HANDLE_DIGITS: usize = 16;

/// The application a `recon ui` command observes, and the instance `--pid` chose, if any.
///
/// An inspection session is deliberately not part of this: a session is an access relationship to
/// a process, not another way of naming the application or the instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selection {
    pub target: String,
    pub pid: Option<Pid>,
}

/// An opaque, session-scoped identity for one live native object: a window, view, layer, cell, or
/// referenced layout guide.
///
/// A handle names the object, not its location: it survives reordering and reparenting, and it
/// goes stale when the object is deallocated or its session ends. The spelling is fixed so a
/// malformed handle is a usage error rather than a lookup that fails later.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct NodeHandle(String);

impl NodeHandle {
    /// Accept a handle written as exactly [`HANDLE_DIGITS`] lowercase hex digits.
    ///
    /// # Errors
    ///
    /// The supplied text when it is not that exact shape.
    pub fn parse(text: &str) -> Result<Self, String> {
        let valid = text.len() == HANDLE_DIGITS
            && text.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(format!("expected {HANDLE_DIGITS} lowercase hex digits"))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NodeHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// How a request asks to reach its target. There is no default and no fallback between modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AccessMode {
    /// Connect to a compatible helper already resident in the selected process.
    Existing,
    /// Start a non-running selected application with the helper enabled.
    Launch,
    /// Load the helper into a selected running process.
    Attach,
}

impl AccessMode {
    pub const ALL: [&'static str; 3] = ["existing", "launch", "attach"];

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "existing" => Some(Self::Existing),
            "launch" => Some(Self::Launch),
            "attach" => Some(Self::Attach),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Existing => "existing",
            Self::Launch => "launch",
            Self::Attach => "attach",
        }
    }
}

impl fmt::Display for AccessMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Which access profile a session runs under. `research` only selects the operator-prepared path;
/// it never prepares a host itself.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AccessProfile {
    #[default]
    Standard,
    Research,
}

impl AccessProfile {
    pub const ALL: [&'static str; 2] = ["standard", "research"];

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "standard" => Some(Self::Standard),
            "research" => Some(Self::Research),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Research => "research",
        }
    }
}

impl fmt::Display for AccessProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Which containment relation a native traversal walks. It selects the relation, not a cast: a
/// layer walk reports layers and a view walk reports views, and neither splices the other in.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum TraversalKind {
    #[default]
    View,
    Layer,
}

impl TraversalKind {
    pub const ALL: [&'static str; 2] = ["view", "layer"];

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "view" => Some(Self::View),
            "layer" => Some(Self::Layer),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::View => "view",
            Self::Layer => "layer",
        }
    }
}

impl fmt::Display for TraversalKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One group of observations a native read may be asked for. Identity, kind, the relationships
/// needed to interpret the structure, and completeness metadata are always reported and are not
/// groups.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FieldGroup {
    Basic,
    Appearance,
    Font,
    Layout,
    Layer,
    Cell,
}

impl FieldGroup {
    pub const ALL: [&'static str; 6] = ["basic", "appearance", "font", "layout", "layer", "cell"];

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "basic" => Some(Self::Basic),
            "appearance" => Some(Self::Appearance),
            "font" => Some(Self::Font),
            "layout" => Some(Self::Layout),
            "layer" => Some(Self::Layer),
            "cell" => Some(Self::Cell),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Basic => "basic",
            Self::Appearance => "appearance",
            Self::Font => "font",
            Self::Layout => "layout",
            Self::Layer => "layer",
            Self::Cell => "cell",
        }
    }
}

impl fmt::Display for FieldGroup {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The groups a native read was asked for: a non-empty set without repeats, in the order the
/// caller wrote it. No group is read merely to discard it later, so the set is also the work the
/// helper is asked to do.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct FieldSet(Vec<FieldGroup>);

impl FieldSet {
    /// The separator between groups in `--fields`.
    pub const SEPARATOR: char = ',';

    /// Accept a comma-separated list of known groups with no repeats and no empty entries.
    ///
    /// # Errors
    ///
    /// A description of the first unknown, repeated, or empty group.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut groups = Vec::new();
        for word in text.split(Self::SEPARATOR) {
            let Some(group) = FieldGroup::parse(word) else {
                return Err(format!(
                    "expected a comma-separated set of {}; {word:?} is not a field group",
                    FieldGroup::ALL.join(", ")
                ));
            };
            if groups.contains(&group) {
                return Err(format!("field group {group} was listed more than once"));
            }
            groups.push(group);
        }
        Ok(Self(groups))
    }

    pub fn groups(&self) -> &[FieldGroup] {
        &self.0
    }

    pub fn contains(&self, group: FieldGroup) -> bool {
        self.0.contains(&group)
    }
}

/// `basic`, the set a native read uses when `--fields` is absent.
impl Default for FieldSet {
    fn default() -> Self {
        Self(vec![FieldGroup::Basic])
    }
}

impl fmt::Display for FieldSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let groups: Vec<&str> = self.0.iter().map(|group| group.as_str()).collect();
        formatter.write_str(&groups.join(","))
    }
}

/// Where a native traversal starts. Without either, a view traversal walks each window's content
/// hierarchy; a layer traversal requires one of them because a layer scope must name an existing
/// window, view, or layer.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeScope {
    /// The window server window id the selected process owns.
    pub window: Option<WindowId>,
    /// A live native root handle from this session.
    pub root: Option<NodeHandle>,
}

/// `ui view find`'s exact filters. They are ANDed, and at least one is required.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Filters {
    pub class: Option<String>,
    pub identifier: Option<String>,
}

impl Filters {
    pub fn is_empty(&self) -> bool {
        self.class.is_none() && self.identifier.is_none()
    }
}

/// What `recon ui session prepare-host` was asked to do. Preparation never happens as a side
/// effect of an inspection command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPreparation {
    /// Disclose the exact changes, ask for confirmation, then set the boot arguments Recon can set.
    Apply,
    /// Report each prerequisite and what is missing, changing nothing.
    Status,
    /// Remove the boot arguments Recon set, changing nothing else.
    Revert,
}

/// One `recon ui` command. Without a `window`, `ax tree` reads the whole application and
/// `screenshot` captures the application's main window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiRequest {
    Windows {
        selection: Selection,
        session: Option<SessionId>,
    },
    AxTree {
        selection: Selection,
        session: Option<SessionId>,
        window: Option<WindowId>,
        depth: Option<usize>,
        no_menu_bar: bool,
    },
    Screenshot {
        selection: Selection,
        session: Option<SessionId>,
        window: Option<WindowId>,
        out: PathBuf,
    },
    Session(SessionRequest),
    View(ViewRequest),
    /// Gather native and external accessibility evidence for the same selected window and report
    /// candidate relationships between them.
    Correlate {
        selection: Selection,
        session: SessionId,
        window: WindowId,
        depth: usize,
        limit: usize,
    },
}

/// One `recon ui session` command: the inspection-access lifecycle, which never closes a window or
/// quits an application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionRequest {
    /// Report access eligibility and unresolved prerequisites without loading code or prompting.
    Preflight { selection: Selection },
    /// Explicit, opt-in research-host preparation.
    PrepareHost { action: HostPreparation },
    /// Establish one explicitly requested session.
    Open {
        selection: Selection,
        mode: AccessMode,
        profile: AccessProfile,
        allow_authorization: bool,
    },
    /// Close and invalidate one session by its identifier.
    Close { session: SessionId },
}

/// One `recon ui view` command: native visual inspection inside an open session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewRequest {
    Tree {
        selection: Selection,
        session: SessionId,
        scope: NativeScope,
        kind: TraversalKind,
        depth: usize,
        fields: FieldSet,
        limit: usize,
    },
    Node {
        selection: Selection,
        session: SessionId,
        handle: NodeHandle,
        fields: FieldSet,
    },
    Find {
        selection: Selection,
        session: SessionId,
        scope: NativeScope,
        kind: TraversalKind,
        filters: Filters,
        depth: usize,
        fields: FieldSet,
        limit: usize,
    },
    Fonts {
        selection: Selection,
        session: SessionId,
        scope: NativeScope,
        depth: usize,
        limit: usize,
    },
}
