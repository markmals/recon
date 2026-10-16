use std::collections::HashMap;
use std::hash::Hash;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use crate::ui::window::Bounds;

/// The root is at depth 0 and the children of a node at this depth are not read.
pub const MAX_DEPTH: usize = 64;
/// A result holds at most this many nodes, counting repeated and failed ones.
pub const MAX_NODES: usize = 25_000;
/// A longer text value is cut to this many Unicode scalar values.
pub const MAX_VALUE_SCALARS: usize = 4_096;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AttributeValue {
    Text(String),
    Integer(i64),
    Real(f64),
    Boolean(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadFailure {
    Unavailable,
    Timeout,
    Failed(i32),
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
    pub children: Vec<E>,
}

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
            children: Vec::new(),
        }
    }
}

/// Where elements come from: the macOS adapter reads them out of a running application.
pub trait ElementSource {
    type Element: Clone + Eq + Hash;

    fn read(
        &mut self,
        element: &Self::Element,
    ) -> Result<ElementAttributes<Self::Element>, ReadFailure>;
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementNode {
    pub role: Option<String>,
    pub subrole: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub value: Option<AttributeValue>,
    #[serde(skip_serializing_if = "is_unset")]
    pub value_truncated: bool,
    pub identifier: Option<String>,
    pub enabled: Option<bool>,
    pub focused: Option<bool>,
    pub frame: Option<Bounds>,
    pub actions: Vec<String>,
    pub children: Vec<Node>,
    #[serde(skip_serializing_if = "is_unset")]
    pub children_omitted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Element(Box<ElementNode>),
    /// An element already emitted in this result, carrying the role it reported then.
    Repeated {
        role: Option<String>,
    },
    Failed(ReadFailure),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tree {
    pub truncated: bool,
    pub root: Node,
}

fn is_unset(flag: &bool) -> bool {
    !flag
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Node::Element(element) => element.serialize(serializer),
            Node::Repeated { role } => {
                let mut node = serializer.serialize_struct("Node", 2)?;
                node.serialize_field("role", role)?;
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

/// Reads an application's accessibility tree depth first, within Recon's reading limits.
pub fn read_tree<S: ElementSource>(source: &mut S, root: S::Element) -> Tree {
    let mut reading = Reading {
        source,
        emitted: HashMap::new(),
        nodes: 0,
        timed_out: false,
        omitted_children: false,
    };
    let root = reading.visit(root, 0);
    Tree { truncated: reading.timed_out || reading.omitted_children, root }
}

struct Reading<'a, S: ElementSource> {
    source: &'a mut S,
    emitted: HashMap<S::Element, Option<String>>,
    nodes: usize,
    timed_out: bool,
    omitted_children: bool,
}

impl<S: ElementSource> Reading<'_, S> {
    fn visit(&mut self, element: S::Element, depth: usize) -> Node {
        self.nodes += 1;
        if let Some(role) = self.emitted.get(&element) {
            return Node::Repeated { role: role.clone() };
        }
        let attributes = match self.source.read(&element) {
            Ok(attributes) => attributes,
            Err(failure) => {
                self.emitted.insert(element, None);
                self.timed_out |= failure == ReadFailure::Timeout;
                return Node::Failed(failure);
            }
        };
        self.emitted.insert(element, attributes.role.clone());

        let (children, children_omitted) = self.visit_children(attributes.children, depth);
        self.omitted_children |= children_omitted;
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
            children,
            children_omitted,
        }))
    }

    /// Returns the children that were read, and whether any child was left unread.
    fn visit_children(&mut self, children: Vec<S::Element>, depth: usize) -> (Vec<Node>, bool) {
        if depth >= MAX_DEPTH {
            return (Vec::new(), !children.is_empty());
        }
        let mut nodes = Vec::new();
        for child in children {
            if self.timed_out || self.nodes >= MAX_NODES {
                return (nodes, true);
            }
            nodes.push(self.visit(child, depth + 1));
        }
        (nodes, false)
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
