//! The vocabulary of a native inspection request: how a request names its scope, how deep and
//! how wide it may go, which groups of observations it asks for, and how it names one live
//! native object.
//!
//! These are the host side of the AppKit extension's action inputs. A handle names an object
//! within its own session and means nothing outside it, and a bound is the work the target is
//! actually asked to do rather than a filter applied afterwards.

use std::fmt;

use recon_window_server_macos::window::WindowId;
use schemars::JsonSchema;
use serde::Serialize;

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
