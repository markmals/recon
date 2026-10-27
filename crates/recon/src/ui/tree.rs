//! Reading an accessibility tree depth first within Recon's limits, independently of where the
//! elements come from.

use std::collections::HashMap;
use std::hash::Hash;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use serde_json::json;

use crate::ui::window::Bounds;

/// The root is at depth 0 and the children of a node at this depth are not read.
pub const MAX_DEPTH: usize = 64;
/// A result holds at most this many nodes, counting repeated and failed ones.
pub const MAX_NODES: usize = 25_000;
/// A longer text value is cut to this many Unicode scalar values.
pub const MAX_VALUE_SCALARS: usize = 4_096;

/// An attribute value Recon reports, keeping the JSON type the application gave it.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum AttributeValue {
    Text(String),
    Integer(i64),
    Real(f64),
    Boolean(bool),
}

/// Why one element could not be read. The tree records the failure in the element's place and
/// keeps going around it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadFailure {
    /// The element no longer exists.
    Unavailable,
    /// The application did not answer within the messaging timeout, which also ends the read.
    Timeout,
    /// Any other failure, carrying the `AXError` code the application returned.
    Failed(i32),
}

/// The children one element lists, as far as the read asked for them.
#[derive(Clone, Debug, PartialEq)]
pub enum ElementChildren<E> {
    /// The children were expanded: the elements the source could read, and whether the element
    /// listed one it could not.
    Read { elements: Vec<E>, unreadable: bool },
    /// The children were left unexpanded because the read stops at this element, so the source
    /// counted the members it was given without examining them.
    Unread { present: bool },
}

// Not derived: deriving would require `E: Default`, which an accessibility element is not.
impl<E> Default for ElementChildren<E> {
    fn default() -> Self {
        Self::Read { elements: Vec::new(), unreadable: false }
    }
}

/// What one accessibility element reports about itself.
#[derive(Clone, Debug, PartialEq)]
pub struct ElementAttributes<E> {
    pub role: Option<String>,
    pub subrole: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub value: Option<AttributeValue>,
    pub identifier: Option<String>,
    pub enabled: Option<bool>,
    pub focused: Option<bool>,
    pub frame: Option<Bounds>,
    pub actions: Vec<String>,
    pub children: ElementChildren<E>,
}

// Not derived: deriving would require `E: Default`, which an accessibility element is not.
impl<E> Default for ElementAttributes<E> {
    fn default() -> Self {
        Self {
            role: None,
            subrole: None,
            title: None,
            description: None,
            value: None,
            identifier: None,
            enabled: None,
            focused: None,
            frame: None,
            actions: Vec::new(),
            children: ElementChildren::default(),
        }
    }
}

/// Where elements come from: the macOS adapter reads them out of a running application.
pub trait ElementSource {
    type Element: Clone + Eq + Hash;

    /// Reads one element's attributes. With `expand_children` it also reads the children the
    /// element lists; without it, it only reports whether the element lists any, leaving the
    /// members unexamined.
    fn read(
        &mut self,
        element: &Self::Element,
        expand_children: bool,
    ) -> Result<ElementAttributes<Self::Element>, ReadFailure>;
}

/// How much of the tree to read. The default reads all of it, within Recon's own limits.
#[derive(Clone, Debug, PartialEq)]
pub struct TreeOptions<E> {
    /// The depth the caller asked for. The children of a node at this depth are not read, and
    /// the node says so with `depth_limited`. Without it only [`MAX_DEPTH`] bounds the walk, and
    /// the cut it makes is a truncation rather than a requested omission.
    pub depth: Option<usize>,
    /// An element to leave unread where it appears among the root's direct children. `recon ui
    /// tree --no-menu-bar` nominates the application's own menu bar.
    pub excluded_root_child: Option<E>,
}

// Not derived: deriving would require `E: Default`, which an accessibility element is not.
impl<E> Default for TreeOptions<E> {
    fn default() -> Self {
        Self { depth: None, excluded_root_child: None }
    }
}

/// An observed element; absent fields, empty arrays and false markers are omitted.
/// Schema overrides exclude nulls that `skip_serializing_if` alone would still permit.
#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ElementNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub subrole: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "is_absent_value")]
    #[schemars(with = "AttributeValue")]
    pub value: Option<AttributeValue>,
    /// Set when `value` was cut to [`MAX_VALUE_SCALARS`].
    #[serde(skip_serializing_if = "is_unset")]
    #[schemars(schema_with = "marker_schema")]
    pub value_truncated: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "String")]
    pub identifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "bool")]
    pub focused: Option<bool>,
    #[serde(skip_serializing_if = "is_absent_frame")]
    #[schemars(with = "Bounds")]
    pub frame: Option<Bounds>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(min = 1))]
    pub actions: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[schemars(length(min = 1))]
    pub children: Vec<Node>,
    /// Set when `children` is incomplete through no choice of the caller's: a limit or a timeout
    /// left some unread, or the element listed a child that could not be read.
    #[serde(skip_serializing_if = "is_unset")]
    #[schemars(schema_with = "marker_schema")]
    pub children_omitted: bool,
    /// Set when the element sits at the depth the caller asked for and lists children, which
    /// Recon therefore did not read.
    #[serde(skip_serializing_if = "is_unset")]
    #[schemars(schema_with = "marker_schema")]
    pub depth_limited: bool,
    /// Set on the root when the element the caller nominated was skipped among its children.
    #[serde(skip_serializing_if = "is_unset")]
    #[schemars(schema_with = "marker_schema")]
    pub menu_bar_excluded: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    /// An element read for this result.
    Element(Box<ElementNode>),
    /// An element already emitted in this result, carrying the role it reported then.
    Repeated { role: Option<String> },
    /// An element that could not be read.
    Failed(ReadFailure),
}

/// The result of [`read_tree`]. It is `truncated` when Recon stopped short of something it set
/// out to read — a limit, a timeout, or a child it could not read — and `narrowed` when the
/// caller's options kept it from reading part of the tree. Both false means the tree is complete.
#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    pub truncated: bool,
    pub narrowed: bool,
    pub root: Node,
}

fn is_unset(flag: &bool) -> bool {
    !flag
}

fn is_absent_value(value: &Option<AttributeValue>) -> bool {
    value
        .as_ref()
        .is_none_or(|value| matches!(value, AttributeValue::Real(number) if !number.is_finite()))
}

fn is_absent_frame(frame: &Option<Bounds>) -> bool {
    frame.as_ref().is_none_or(|frame| {
        !frame.x.is_finite()
            || !frame.y.is_finite()
            || !frame.width.is_finite()
            || !frame.height.is_finite()
    })
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Node::Element(element) => element.serialize(serializer),
            Node::Repeated { role } => {
                let mut node =
                    serializer.serialize_struct("Node", 1 + usize::from(role.is_some()))?;
                if let Some(role) = role {
                    node.serialize_field("role", role)?;
                }
                node.serialize_field("repeated", &true)?;
                node.end()
            }
            Node::Failed(ReadFailure::Unavailable) => {
                serialize_failure(serializer, "element-unavailable", None)
            }
            Node::Failed(ReadFailure::Timeout) => serialize_failure(serializer, "timeout", None),
            Node::Failed(ReadFailure::Failed(code)) => {
                serialize_failure(serializer, "read-failed", Some(*code))
            }
        }
    }
}

/// Each branch describes exactly one shape [`Serialize`] writes. The branches stay disjoint
/// because every one of them is closed: an element node, which has no required field at all and
/// so can be written as `{}`, rejects the `repeated` and `error` keys that distinguish the others.
impl JsonSchema for Node {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Node".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let element = generator.subschema_for::<ElementNode>();
        serde_json::from_value(json!({"oneOf":[
            element,
            {"type":"object","properties":{"role":{"type":"string"},"repeated":{"const":true}},"required":["repeated"],"additionalProperties":false},
            {"type":"object","properties":{"error":{"enum":["element-unavailable","timeout"]}},"required":["error"],"additionalProperties":false},
            {"type":"object","properties":{"error":{"const":"read-failed"},"axError":{"type":"integer","minimum":i32::MIN,"maximum":i32::MAX}},"required":["error","axError"],"additionalProperties":false}
        ]})).expect("valid node schema")
    }
}

/// A marker is written only when it is set, so `true` is the only value a result can carry.
fn marker_schema(_: &mut SchemaGenerator) -> Schema {
    serde_json::from_value(json!({"const":true})).expect("valid marker schema")
}

fn serialize_failure<S: Serializer>(
    serializer: S,
    error: &str,
    ax_error: Option<i32>,
) -> Result<S::Ok, S::Error> {
    let mut node = serializer.serialize_struct("Node", 1 + usize::from(ax_error.is_some()))?;
    node.serialize_field("error", error)?;
    if let Some(code) = ax_error {
        node.serialize_field("axError", &code)?;
    }
    node.end()
}

/// Reads an application's accessibility tree depth first, within Recon's reading limits and
/// whatever the caller asked to leave out.
pub fn read_tree<S: ElementSource>(
    source: &mut S,
    root: S::Element,
    options: TreeOptions<S::Element>,
) -> Tree {
    let mut reading = Reading {
        source,
        options,
        emitted: HashMap::new(),
        nodes: 0,
        timed_out: false,
        omitted_children: false,
        requested_omission: false,
    };
    let root = reading.visit(root, 0);
    Tree {
        truncated: reading.timed_out || reading.omitted_children,
        narrowed: reading.requested_omission,
        root,
    }
}

struct Reading<'a, S: ElementSource> {
    source: &'a mut S,
    options: TreeOptions<S::Element>,
    emitted: HashMap<S::Element, Option<String>>,
    nodes: usize,
    timed_out: bool,
    omitted_children: bool,
    requested_omission: bool,
}

/// What the walk made of one element's children.
struct Listing {
    nodes: Vec<Node>,
    children_omitted: bool,
    depth_limited: bool,
    menu_bar_excluded: bool,
}

impl<S: ElementSource> Reading<'_, S> {
    fn visit(&mut self, element: S::Element, depth: usize) -> Node {
        self.nodes += 1;
        if let Some(role) = self.emitted.get(&element) {
            return Node::Repeated { role: role.clone() };
        }
        let expand_children = self.expands_children(depth);
        let attributes = match self.source.read(&element, expand_children) {
            Ok(attributes) => attributes,
            Err(failure) => {
                self.emitted.insert(element, None);
                self.timed_out |= failure == ReadFailure::Timeout;
                return Node::Failed(failure);
            }
        };
        self.emitted.insert(element, attributes.role.clone());

        let listing = self.read_children(attributes.children, depth);
        self.omitted_children |= listing.children_omitted;
        self.requested_omission |= listing.depth_limited || listing.menu_bar_excluded;
        let (value, value_truncated) = cut_long_text(attributes.value);
        Node::Element(Box::new(ElementNode {
            role: attributes.role,
            subrole: attributes.subrole,
            title: attributes.title,
            description: attributes.description,
            value,
            value_truncated,
            identifier: attributes.identifier,
            enabled: attributes.enabled,
            focused: attributes.focused,
            frame: attributes.frame,
            actions: attributes.actions,
            children: listing.nodes,
            children_omitted: listing.children_omitted,
            depth_limited: listing.depth_limited,
            menu_bar_excluded: listing.menu_bar_excluded,
        }))
    }

    /// A node at the depth the caller asked for is read without its children, so the source never
    /// looks at the members it would have walked.
    fn expands_children(&self, depth: usize) -> bool {
        match self.options.depth {
            Some(limit) => depth < limit,
            None => true,
        }
    }

    fn read_children(&mut self, children: ElementChildren<S::Element>, depth: usize) -> Listing {
        let (elements, unreadable) = match children {
            // The caller's depth stopped this node, so whatever it lists is a requested omission.
            ElementChildren::Unread { present } => {
                return Listing {
                    nodes: Vec::new(),
                    children_omitted: false,
                    depth_limited: present,
                    menu_bar_excluded: false,
                };
            }
            ElementChildren::Read { elements, unreadable } => (elements, unreadable),
        };
        // Recon's own cap, which a requested depth never reaches past.
        if depth >= MAX_DEPTH {
            return Listing {
                nodes: Vec::new(),
                children_omitted: unreadable || !elements.is_empty(),
                depth_limited: false,
                menu_bar_excluded: false,
            };
        }

        let mut nodes = Vec::new();
        let mut left_unread = unreadable;
        let mut excluded = false;
        for child in elements {
            // Skipping costs no request and no budget, and leaves nothing in the child's place.
            if depth == 0 && self.options.excluded_root_child.as_ref() == Some(&child) {
                excluded = true;
                continue;
            }
            if self.timed_out || self.nodes >= MAX_NODES {
                left_unread = true;
                break;
            }
            nodes.push(self.visit(child, depth + 1));
        }
        Listing {
            nodes,
            children_omitted: left_unread,
            depth_limited: false,
            menu_bar_excluded: excluded,
        }
    }
}

fn cut_long_text(value: Option<AttributeValue>) -> (Option<AttributeValue>, bool) {
    let text = match value {
        Some(AttributeValue::Text(text)) => text,
        other => return (other, false),
    };
    match text.char_indices().nth(MAX_VALUE_SCALARS) {
        Some((cut, _)) => (Some(AttributeValue::Text(text[..cut].to_owned())), true),
        None => (Some(AttributeValue::Text(text)), false),
    }
}
