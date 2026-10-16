use std::collections::HashMap;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use recon::ui::tree::{
    AttributeValue, ElementAttributes, ElementNode, ElementSource, MAX_DEPTH, MAX_NODES,
    MAX_VALUE_SCALARS, Node, ReadFailure, Tree, read_tree,
};
use recon::ui::window::Bounds;

#[derive(Default)]
struct RecordedSource {
    elements: HashMap<u32, Result<ElementAttributes<u32>, ReadFailure>>,
    reads: Vec<u32>,
}

impl RecordedSource {
    fn with(mut self, element: u32, role: &str, children: &[u32]) -> Self {
        self.elements.insert(
            element,
            Ok(ElementAttributes {
                role: Some(role.to_owned()),
                children: children.to_vec(),
                ..ElementAttributes::default()
            }),
        );
        self
    }

    fn failing(mut self, element: u32, failure: ReadFailure) -> Self {
        self.elements.insert(element, Err(failure));
        self
    }

    /// Marks an element that listed a child the source could not read, as the macOS adapter does
    /// for an `AXChildren` member that is not an accessibility element.
    fn unreadable_children(mut self, element: u32) -> Self {
        if let Some(Ok(attributes)) = self.elements.get_mut(&element) {
            attributes.unreadable_children = true;
        }
        self
    }
}

impl ElementSource for RecordedSource {
    type Element = u32;

    fn read(&mut self, element: &u32) -> Result<ElementAttributes<u32>, ReadFailure> {
        self.reads.push(*element);
        self.elements.get(element).cloned().unwrap_or(Err(ReadFailure::Unavailable))
    }
}

fn element_of(node: &Node) -> &ElementNode {
    match node {
        Node::Element(element) => element,
        other => panic!("expected an element node, got {other:?}"),
    }
}

fn roles(node: &Node) -> Vec<Option<String>> {
    let mut collected = Vec::new();
    collect_roles(node, &mut collected);
    collected
}

fn collect_roles(node: &Node, collected: &mut Vec<Option<String>>) {
    match node {
        Node::Element(element) => {
            collected.push(element.role.clone());
            for child in &element.children {
                collect_roles(child, collected);
            }
        }
        Node::Repeated { role } => collected.push(role.clone()),
        Node::Failed(_) => collected.push(None),
    }
}

fn count_nodes(node: &Node) -> usize {
    match node {
        Node::Element(element) => 1 + element.children.iter().map(count_nodes).sum::<usize>(),
        _ => 1,
    }
}

fn deepest(node: &Node) -> usize {
    match node {
        Node::Element(element) => 1 + element.children.iter().map(deepest).max().unwrap_or(0),
        _ => 1,
    }
}

fn omits_children(node: &Node) -> bool {
    match node {
        Node::Element(element) => {
            element.children_omitted || element.children.iter().any(omits_children)
        }
        _ => false,
    }
}

#[test]
fn children_are_read_depth_first_in_the_order_the_application_reports() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2, 3])
        .with(2, "AXMenuBar", &[4])
        .with(3, "AXWindow", &[])
        .with(4, "AXMenu", &[]);

    let tree = read_tree(&mut source, 1);

    assert_eq!(source.reads, vec![1, 2, 4, 3]);
    assert_eq!(
        roles(&tree.root),
        vec![
            Some("AXApplication".to_owned()),
            Some("AXMenuBar".to_owned()),
            Some("AXMenu".to_owned()),
            Some("AXWindow".to_owned()),
        ]
    );
    assert!(!tree.truncated);
    assert!(!element_of(&tree.root).children_omitted);
}

#[test]
fn every_attribute_the_application_reports_reaches_the_node() {
    let mut source = RecordedSource::default();
    source.elements.insert(
        1,
        Ok(ElementAttributes {
            role: Some("AXTextField".to_owned()),
            subrole: Some("AXSearchField".to_owned()),
            title: Some("Search".to_owned()),
            description: Some("search field".to_owned()),
            value: Some(AttributeValue::Text("fixture value".to_owned())),
            identifier: Some("search".to_owned()),
            enabled: Some(true),
            focused: Some(false),
            frame: Some(Bounds { x: 1.0, y: 2.0, width: 3.0, height: 4.0 }),
            actions: vec!["AXPress".to_owned()],
            children: Vec::new(),
            unreadable_children: false,
        }),
    );

    let tree = read_tree(&mut source, 1);
    let node = element_of(&tree.root);

    assert_eq!(node.subrole.as_deref(), Some("AXSearchField"));
    assert_eq!(node.title.as_deref(), Some("Search"));
    assert_eq!(node.description.as_deref(), Some("search field"));
    assert_eq!(node.value, Some(AttributeValue::Text("fixture value".to_owned())));
    assert_eq!(node.identifier.as_deref(), Some("search"));
    assert_eq!(node.enabled, Some(true));
    assert_eq!(node.focused, Some(false));
    assert_eq!(node.frame.map(|frame| frame.width), Some(3.0));
    assert_eq!(node.actions, vec!["AXPress".to_owned()]);
    assert!(!node.value_truncated);
}

#[test]
fn the_children_of_a_node_at_depth_64_are_not_read() {
    let mut source = RecordedSource::default();
    for element in 1..=200u32 {
        source = source.with(element, "AXGroup", &[element + 1]);
    }

    let tree = read_tree(&mut source, 1);

    assert_eq!(source.reads.len(), MAX_DEPTH + 1);
    assert_eq!(*source.reads.last().unwrap(), MAX_DEPTH as u32 + 1);
    assert_eq!(count_nodes(&tree.root), MAX_DEPTH + 1);
    assert_eq!(deepest(&tree.root), MAX_DEPTH + 1);
    assert!(tree.truncated);

    let mut node = &tree.root;
    for _ in 0..MAX_DEPTH {
        let element = element_of(node);
        assert!(!element.children_omitted);
        node = &element.children[0];
    }
    assert!(element_of(node).children_omitted);
    assert!(element_of(node).children.is_empty());
}

#[test]
fn a_result_holds_at_most_25000_nodes() {
    let children = (2..=40_000u32).collect::<Vec<_>>();
    let mut source = RecordedSource::default().with(1, "AXApplication", &children);
    for child in &children {
        source = source.with(*child, "AXGroup", &[]);
    }

    let tree = read_tree(&mut source, 1);

    assert_eq!(count_nodes(&tree.root), MAX_NODES);
    assert_eq!(source.reads.len(), MAX_NODES);
    assert!(element_of(&tree.root).children_omitted);
    assert!(tree.truncated);
}

#[test]
fn an_element_already_emitted_is_repeated_without_being_read_again() {
    let mut source =
        RecordedSource::default().with(1, "AXApplication", &[2]).with(2, "AXWindow", &[1, 2]);

    let tree = read_tree(&mut source, 1);

    assert_eq!(source.reads, vec![1, 2]);
    let window = element_of(&element_of(&tree.root).children[0]);
    assert_eq!(
        window.children,
        vec![
            Node::Repeated { role: Some("AXApplication".to_owned()) },
            Node::Repeated { role: Some("AXWindow".to_owned()) },
        ]
    );
    assert!(!tree.truncated);
    assert_eq!(count_nodes(&tree.root), 4);
}

#[test]
fn an_element_that_no_longer_exists_becomes_an_unavailable_node() {
    let mut source =
        RecordedSource::default().with(1, "AXApplication", &[2, 3]).with(3, "AXWindow", &[]);

    let tree = read_tree(&mut source, 1);

    let children = &element_of(&tree.root).children;
    assert_eq!(children[0], Node::Failed(ReadFailure::Unavailable));
    assert_eq!(element_of(&children[1]).role.as_deref(), Some("AXWindow"));
    assert!(!tree.truncated);
    assert!(!element_of(&tree.root).children_omitted);
}

#[test]
fn a_failed_read_carries_the_accessibility_error_code() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2])
        .failing(2, ReadFailure::Failed(-25_200));

    let tree = read_tree(&mut source, 1);

    assert_eq!(element_of(&tree.root).children[0], Node::Failed(ReadFailure::Failed(-25_200)));
    assert!(!tree.truncated);
}

#[test]
fn a_timeout_ends_reading_and_truncates_the_tree() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2, 3, 4])
        .with(2, "AXMenuBar", &[])
        .failing(3, ReadFailure::Timeout)
        .with(4, "AXWindow", &[]);

    let tree = read_tree(&mut source, 1);

    assert_eq!(source.reads, vec![1, 2, 3]);
    let root = element_of(&tree.root);
    assert_eq!(root.children.len(), 2);
    assert_eq!(root.children[1], Node::Failed(ReadFailure::Timeout));
    assert!(root.children_omitted);
    assert!(tree.truncated);
}

#[test]
fn a_timeout_truncates_even_when_no_children_remain() {
    let mut source =
        RecordedSource::default().with(1, "AXApplication", &[2]).failing(2, ReadFailure::Timeout);

    let tree = read_tree(&mut source, 1);

    assert_eq!(source.reads, vec![1, 2]);
    assert!(!element_of(&tree.root).children_omitted);
    assert!(tree.truncated);
}

#[test]
fn a_child_the_source_could_not_read_is_reported_as_an_omitted_child() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2])
        .with(2, "AXWindow", &[])
        .unreadable_children(1);

    let tree = read_tree(&mut source, 1);

    let root = element_of(&tree.root);
    assert_eq!(root.children.len(), 1);
    assert!(root.children_omitted);
    assert!(!element_of(&root.children[0]).children_omitted);
    assert!(tree.truncated);
}

#[test]
fn a_string_value_longer_than_4096_scalars_is_cut_and_flagged() {
    let long = "é".repeat(MAX_VALUE_SCALARS + 10);
    let exact = "a".repeat(MAX_VALUE_SCALARS);
    let mut source = RecordedSource::default();
    source.elements.insert(
        1,
        Ok(ElementAttributes {
            value: Some(AttributeValue::Text(long.clone())),
            children: vec![2, 3],
            ..ElementAttributes::default()
        }),
    );
    source.elements.insert(
        2,
        Ok(ElementAttributes {
            value: Some(AttributeValue::Text(exact.clone())),
            ..ElementAttributes::default()
        }),
    );
    source.elements.insert(
        3,
        Ok(ElementAttributes {
            value: Some(AttributeValue::Integer(9)),
            ..ElementAttributes::default()
        }),
    );

    let tree = read_tree(&mut source, 1);
    let root = element_of(&tree.root);

    assert_eq!(root.value, Some(AttributeValue::Text("é".repeat(MAX_VALUE_SCALARS))));
    assert!(root.value_truncated);
    assert!(!tree.truncated, "a cut value is not a truncated tree");

    let unchanged = element_of(&root.children[0]);
    assert_eq!(unchanged.value, Some(AttributeValue::Text(exact)));
    assert!(!unchanged.value_truncated);

    let number = element_of(&root.children[1]);
    assert_eq!(number.value, Some(AttributeValue::Integer(9)));
    assert!(!number.value_truncated);
}

struct GeneratedTree {
    branching: usize,
    depth_limit: usize,
    depth_of: HashMap<u64, usize>,
    next_element: u64,
}

impl GeneratedTree {
    fn new(branching: usize, depth_limit: usize) -> Self {
        Self { branching, depth_limit, depth_of: HashMap::from([(0, 0)]), next_element: 1 }
    }

    fn expected_nodes(&self) -> usize {
        let mut total = 0usize;
        let mut level = 1usize;
        for _ in 0..=self.depth_limit {
            total += level;
            level *= self.branching;
        }
        total
    }
}

impl ElementSource for GeneratedTree {
    type Element = u64;

    fn read(&mut self, element: &u64) -> Result<ElementAttributes<u64>, ReadFailure> {
        let depth = self.depth_of[element];
        let mut children = Vec::new();
        if depth < self.depth_limit {
            for _ in 0..self.branching {
                let child = self.next_element;
                self.next_element += 1;
                self.depth_of.insert(child, depth + 1);
                children.push(child);
            }
        }
        Ok(ElementAttributes {
            role: Some("AXGroup".to_owned()),
            children,
            ..ElementAttributes::default()
        })
    }
}

fn is_truncated_consistently(tree: &Tree) -> bool {
    tree.truncated == omits_children(&tree.root)
}

proptest! {
    #[test]
    fn a_tree_that_fits_is_reported_whole(branching in 1usize..5, depth_limit in 0usize..6) {
        let mut source = GeneratedTree::new(branching, depth_limit);
        let expected = source.expected_nodes();
        prop_assume!(expected <= MAX_NODES);

        let tree = read_tree(&mut source, 0);

        prop_assert_eq!(count_nodes(&tree.root), expected);
        prop_assert_eq!(deepest(&tree.root), depth_limit + 1);
        prop_assert!(!tree.truncated);
        prop_assert!(is_truncated_consistently(&tree));
    }
}

proptest! {
    // Each case builds 25,000 nodes, and the strategy has only three distinct inputs.
    #![proptest_config(ProptestConfig::with_cases(8))]

    #[test]
    fn a_tree_that_does_not_fit_stops_at_the_limit(branching in 2usize..5) {
        let mut source = GeneratedTree::new(branching, 1_000);

        let tree = read_tree(&mut source, 0);

        prop_assert_eq!(count_nodes(&tree.root), MAX_NODES);
        prop_assert!(deepest(&tree.root) <= MAX_DEPTH + 1);
        prop_assert!(tree.truncated);
        prop_assert!(is_truncated_consistently(&tree));
    }
}

proptest! {

    #[test]
    fn a_text_value_is_cut_to_a_prefix_of_at_most_4096_scalars(
        text in prop::collection::vec(any::<char>(), 0..5_000)
    ) {
        let text: String = text.into_iter().collect();
        let scalars = text.chars().count();
        let mut source = RecordedSource::default();
        source.elements.insert(
            1,
            Ok(ElementAttributes {
                value: Some(AttributeValue::Text(text.clone())),
                ..ElementAttributes::default()
            }),
        );

        let tree = read_tree(&mut source, 1);
        let node = element_of(&tree.root);
        let Some(AttributeValue::Text(reported)) = node.value.clone() else {
            return Err(TestCaseError::fail("a text value stays a text value"));
        };

        prop_assert!(text.starts_with(&reported));
        prop_assert_eq!(reported.chars().count(), scalars.min(MAX_VALUE_SCALARS));
        prop_assert_eq!(node.value_truncated, scalars > MAX_VALUE_SCALARS);
    }
}
