use std::collections::HashMap;

use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use recon::ui::tree::{
    AttributeValue, ElementAttributes, ElementChildren, ElementNode, ElementSource, MAX_DEPTH,
    MAX_NODES, MAX_VALUE_SCALARS, Node, ReadFailure, Tree, TreeOptions, read_tree,
};
use recon::ui::window::Bounds;

#[derive(Default)]
struct RecordedSource {
    elements: HashMap<u32, Result<ElementAttributes<u32>, ReadFailure>>,
    reads: Vec<u32>,
    /// The elements read without expanding their children, in the order they were read.
    unexpanded_reads: Vec<u32>,
}

impl RecordedSource {
    fn with(mut self, element: u32, role: &str, children: &[u32]) -> Self {
        self.elements.insert(
            element,
            Ok(ElementAttributes {
                role: Some(role.to_owned()),
                children: ElementChildren::Read { elements: children.to_vec(), unreadable: false },
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
        if let Some(Ok(attributes)) = self.elements.get_mut(&element)
            && let ElementChildren::Read { unreadable, .. } = &mut attributes.children
        {
            *unreadable = true;
        }
        self
    }
}

impl ElementSource for RecordedSource {
    type Element = u32;

    fn read(
        &mut self,
        element: &u32,
        expand_children: bool,
    ) -> Result<ElementAttributes<u32>, ReadFailure> {
        self.reads.push(*element);
        if !expand_children {
            self.unexpanded_reads.push(*element);
        }
        let mut attributes =
            self.elements.get(element).cloned().unwrap_or(Err(ReadFailure::Unavailable))?;
        if !expand_children {
            attributes.children = ElementChildren::Unread { present: lists_children(&attributes) };
        }
        Ok(attributes)
    }
}

/// Without expanding, a source counts the members of the children attribute without examining
/// them, so a member it could not have read still counts as present.
fn lists_children(attributes: &ElementAttributes<u32>) -> bool {
    match &attributes.children {
        ElementChildren::Read { elements, unreadable } => !elements.is_empty() || *unreadable,
        ElementChildren::Unread { present } => *present,
    }
}

fn whole_tree() -> TreeOptions<u32> {
    TreeOptions::default()
}

fn to_depth(limit: usize) -> TreeOptions<u32> {
    TreeOptions { depth: Some(limit), ..TreeOptions::default() }
}

fn excluding(element: u32) -> TreeOptions<u32> {
    TreeOptions { excluded_root_child: Some(element), ..TreeOptions::default() }
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

fn leaves_out_what_was_asked_for(node: &Node) -> bool {
    match node {
        Node::Element(element) => {
            element.depth_limited
                || element.menu_bar_excluded
                || element.children.iter().any(leaves_out_what_was_asked_for)
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

    let tree = read_tree(&mut source, 1, whole_tree());

    assert_eq!(source.reads, vec![1, 2, 4, 3]);
    assert!(source.unexpanded_reads.is_empty(), "an unbounded read expands every element");
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
    assert!(!tree.narrowed, "a tree nobody narrowed is not narrowed");
    let root = element_of(&tree.root);
    assert!(!root.children_omitted);
    assert!(!root.depth_limited);
    assert!(!root.menu_bar_excluded);
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
            children: ElementChildren::Read { elements: Vec::new(), unreadable: false },
        }),
    );

    let tree = read_tree(&mut source, 1, whole_tree());
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

    let tree = read_tree(&mut source, 1, whole_tree());

    assert_eq!(source.reads.len(), MAX_DEPTH + 1);
    assert_eq!(*source.reads.last().unwrap(), MAX_DEPTH as u32 + 1);
    assert_eq!(count_nodes(&tree.root), MAX_DEPTH + 1);
    assert_eq!(deepest(&tree.root), MAX_DEPTH + 1);
    assert!(tree.truncated);
    assert!(!tree.narrowed, "the safety cap is a cut Recon made, not one the caller asked for");

    let mut node = &tree.root;
    for _ in 0..MAX_DEPTH {
        let element = element_of(node);
        assert!(!element.children_omitted);
        node = &element.children[0];
    }
    assert!(element_of(node).children_omitted);
    assert!(!element_of(node).depth_limited);
    assert!(element_of(node).children.is_empty());
}

#[test]
fn a_result_holds_at_most_25000_nodes() {
    let children = (2..=40_000u32).collect::<Vec<_>>();
    let mut source = RecordedSource::default().with(1, "AXApplication", &children);
    for child in &children {
        source = source.with(*child, "AXGroup", &[]);
    }

    let tree = read_tree(&mut source, 1, whole_tree());

    assert_eq!(count_nodes(&tree.root), MAX_NODES);
    assert_eq!(source.reads.len(), MAX_NODES);
    assert!(element_of(&tree.root).children_omitted);
    assert!(tree.truncated);
    assert!(!tree.narrowed);
}

#[test]
fn an_element_already_emitted_is_repeated_without_being_read_again() {
    let mut source =
        RecordedSource::default().with(1, "AXApplication", &[2]).with(2, "AXWindow", &[1, 2]);

    let tree = read_tree(&mut source, 1, whole_tree());

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
    assert!(!tree.narrowed, "an element seen twice is not an element left unread");
    assert_eq!(count_nodes(&tree.root), 4);
}

#[test]
fn an_element_that_no_longer_exists_becomes_an_unavailable_node() {
    let mut source =
        RecordedSource::default().with(1, "AXApplication", &[2, 3]).with(3, "AXWindow", &[]);

    let tree = read_tree(&mut source, 1, whole_tree());

    let children = &element_of(&tree.root).children;
    assert_eq!(children[0], Node::Failed(ReadFailure::Unavailable));
    assert_eq!(element_of(&children[1]).role.as_deref(), Some("AXWindow"));
    assert!(!tree.truncated);
    assert!(!tree.narrowed);
    assert!(!element_of(&tree.root).children_omitted);
}

#[test]
fn a_failed_read_carries_the_accessibility_error_code() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2])
        .failing(2, ReadFailure::Failed(-25_200));

    let tree = read_tree(&mut source, 1, whole_tree());

    assert_eq!(element_of(&tree.root).children[0], Node::Failed(ReadFailure::Failed(-25_200)));
    assert!(!tree.truncated);
    assert!(!tree.narrowed);
}

#[test]
fn a_timeout_ends_reading_and_truncates_the_tree() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2, 3, 4])
        .with(2, "AXMenuBar", &[])
        .failing(3, ReadFailure::Timeout)
        .with(4, "AXWindow", &[]);

    let tree = read_tree(&mut source, 1, whole_tree());

    assert_eq!(source.reads, vec![1, 2, 3]);
    let root = element_of(&tree.root);
    assert_eq!(root.children.len(), 2);
    assert_eq!(root.children[1], Node::Failed(ReadFailure::Timeout));
    assert!(root.children_omitted);
    assert!(tree.truncated);
    assert!(!tree.narrowed);
}

#[test]
fn a_timeout_truncates_even_when_no_children_remain() {
    let mut source =
        RecordedSource::default().with(1, "AXApplication", &[2]).failing(2, ReadFailure::Timeout);

    let tree = read_tree(&mut source, 1, whole_tree());

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

    let tree = read_tree(&mut source, 1, whole_tree());

    let root = element_of(&tree.root);
    assert_eq!(root.children.len(), 1);
    assert!(root.children_omitted);
    assert!(!root.depth_limited);
    assert!(!element_of(&root.children[0]).children_omitted);
    assert!(tree.truncated);
    assert!(!tree.narrowed);
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
            children: ElementChildren::Read { elements: vec![2, 3], unreadable: false },
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

    let tree = read_tree(&mut source, 1, whole_tree());
    let root = element_of(&tree.root);

    assert_eq!(root.value, Some(AttributeValue::Text("é".repeat(MAX_VALUE_SCALARS))));
    assert!(root.value_truncated);
    assert!(!tree.truncated, "a cut value is not a truncated tree");
    assert!(!tree.narrowed, "a cut value is not a narrowed tree");

    let unchanged = element_of(&root.children[0]);
    assert_eq!(unchanged.value, Some(AttributeValue::Text(exact)));
    assert!(!unchanged.value_truncated);

    let number = element_of(&root.children[1]);
    assert_eq!(number.value, Some(AttributeValue::Integer(9)));
    assert!(!number.value_truncated);
}

#[test]
fn a_requested_depth_stops_the_walk_without_examining_the_children_it_leaves_out() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2])
        .with(2, "AXWindow", &[3])
        .with(3, "AXGroup", &[4])
        .with(4, "AXButton", &[]);

    let tree = read_tree(&mut source, 1, to_depth(2));

    assert_eq!(source.reads, vec![1, 2, 3], "nothing below the requested depth is read");
    assert_eq!(source.unexpanded_reads, vec![3], "only the deepest node is read without children");
    let group = element_of(&element_of(&element_of(&tree.root).children[0]).children[0]);
    assert!(group.children.is_empty());
    assert!(group.depth_limited);
    assert!(!group.children_omitted, "a cut the caller asked for is not a truncation");
    assert!(tree.narrowed);
    assert!(!tree.truncated);
}

#[test]
fn a_requested_depth_of_zero_returns_the_root_alone() {
    let mut source =
        RecordedSource::default().with(1, "AXApplication", &[2]).with(2, "AXWindow", &[]);

    let tree = read_tree(&mut source, 1, to_depth(0));

    assert_eq!(source.reads, vec![1]);
    assert_eq!(source.unexpanded_reads, vec![1]);
    let root = element_of(&tree.root);
    assert_eq!(root.role.as_deref(), Some("AXApplication"));
    assert!(root.children.is_empty());
    assert!(root.depth_limited);
    assert!(!root.children_omitted);
    assert!(tree.narrowed);
    assert!(!tree.truncated);
}

#[test]
fn a_node_at_the_requested_depth_with_no_children_carries_no_marker() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2, 3])
        .with(2, "AXWindow", &[])
        .with(3, "AXMenuBar", &[4])
        .with(4, "AXMenu", &[]);

    let tree = read_tree(&mut source, 1, to_depth(1));

    let root = element_of(&tree.root);
    let window = element_of(&root.children[0]);
    let menu_bar = element_of(&root.children[1]);
    assert!(!window.depth_limited, "nothing was omitted below a node with no children");
    assert!(menu_bar.depth_limited);
    assert!(!root.depth_limited);
    assert!(tree.narrowed);
    assert!(!tree.truncated);
}

#[test]
fn a_requested_depth_deeper_than_the_tree_narrows_nothing() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2])
        .with(2, "AXWindow", &[3])
        .with(3, "AXButton", &[]);

    let tree = read_tree(&mut source, 1, to_depth(MAX_DEPTH));

    assert_eq!(source.reads, vec![1, 2, 3]);
    assert_eq!(count_nodes(&tree.root), 3);
    assert!(!tree.narrowed, "a depth the tree never reaches leaves nothing unread");
    assert!(!tree.truncated);
    assert!(!leaves_out_what_was_asked_for(&tree.root));
}

#[test]
fn a_child_the_source_could_not_read_at_the_requested_depth_is_part_of_the_omission() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2])
        .with(2, "AXWindow", &[])
        .unreadable_children(2);

    let tree = read_tree(&mut source, 1, to_depth(1));

    let window = element_of(&element_of(&tree.root).children[0]);
    assert!(window.depth_limited);
    assert!(!window.children_omitted, "the members of the children attribute were not examined");
    assert!(tree.narrowed);
    assert!(!tree.truncated);
}

#[test]
fn a_requested_depth_at_the_safety_cap_marks_the_cut_as_requested() {
    let mut source = RecordedSource::default();
    for element in 1..=200u32 {
        source = source.with(element, "AXGroup", &[element + 1]);
    }

    let tree = read_tree(&mut source, 1, to_depth(MAX_DEPTH));

    assert_eq!(source.reads.len(), MAX_DEPTH + 1);
    assert_eq!(source.unexpanded_reads, vec![MAX_DEPTH as u32 + 1]);
    let mut node = &tree.root;
    for _ in 0..MAX_DEPTH {
        node = &element_of(node).children[0];
    }
    assert!(element_of(node).depth_limited);
    assert!(!element_of(node).children_omitted);
    assert!(tree.narrowed);
    assert!(!tree.truncated);
}

#[test]
fn a_requested_depth_beyond_the_safety_cap_still_stops_at_the_cap() {
    let mut source = RecordedSource::default();
    for element in 1..=200u32 {
        source = source.with(element, "AXGroup", &[element + 1]);
    }

    let tree = read_tree(&mut source, 1, to_depth(MAX_DEPTH + 10));

    assert_eq!(source.reads.len(), MAX_DEPTH + 1);
    assert!(source.unexpanded_reads.is_empty());
    let mut node = &tree.root;
    for _ in 0..MAX_DEPTH {
        node = &element_of(node).children[0];
    }
    assert!(element_of(node).children_omitted, "the cap is Recon's own cut, not the caller's");
    assert!(!element_of(node).depth_limited);
    assert!(tree.truncated);
    assert!(!tree.narrowed);
}

#[test]
fn the_node_budget_still_applies_within_a_requested_depth() {
    let children = (2..=40_000u32).collect::<Vec<_>>();
    let mut source = RecordedSource::default().with(1, "AXApplication", &children);
    for child in &children {
        source = source.with(*child, "AXGroup", &[1]);
    }

    let tree = read_tree(&mut source, 1, to_depth(1));

    assert_eq!(count_nodes(&tree.root), MAX_NODES);
    let root = element_of(&tree.root);
    assert!(root.children_omitted, "the budget ran out before the children did");
    assert!(element_of(&root.children[0]).depth_limited);
    assert!(tree.truncated);
    assert!(tree.narrowed, "a tree can be both narrowed and truncated");
}

#[test]
fn the_nominated_child_is_not_charged_against_the_node_budget() {
    let children = (2..=40_000u32).collect::<Vec<_>>();
    let mut source = RecordedSource::default().with(1, "AXApplication", &children);
    for child in &children {
        source = source.with(*child, "AXGroup", &[]);
    }

    let tree = read_tree(&mut source, 1, excluding(2));

    assert!(!source.reads.contains(&2), "the skipped element is never read");
    assert_eq!(count_nodes(&tree.root), MAX_NODES, "the budget counts what Recon read");
    assert_eq!(source.reads[1], 3, "the children after it keep their order");
    let root = element_of(&tree.root);
    assert!(root.menu_bar_excluded);
    assert!(root.children_omitted, "the budget ran out before the children did");
    assert!(tree.truncated);
    assert!(tree.narrowed);
}

#[test]
fn a_timeout_inside_a_requested_depth_truncates_a_narrowed_tree() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2, 3])
        .with(2, "AXWindow", &[4])
        .failing(3, ReadFailure::Timeout)
        .with(4, "AXGroup", &[5])
        .with(5, "AXButton", &[]);

    let tree = read_tree(&mut source, 1, to_depth(2));

    assert_eq!(source.reads, vec![1, 2, 4, 3]);
    let root = element_of(&tree.root);
    assert_eq!(root.children[1], Node::Failed(ReadFailure::Timeout));
    assert!(element_of(&element_of(&root.children[0]).children[0]).depth_limited);
    assert!(tree.truncated);
    assert!(tree.narrowed);
}

#[test]
fn the_nominated_root_child_is_skipped_without_being_read() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2, 3])
        .with(2, "AXMenuBar", &[4])
        .with(3, "AXWindow", &[])
        .with(4, "AXMenu", &[]);

    let tree = read_tree(&mut source, 1, excluding(2));

    assert_eq!(source.reads, vec![1, 3], "nothing below the nominated element is read");
    assert_eq!(count_nodes(&tree.root), 2, "the skipped element costs nothing and leaves nothing");
    let root = element_of(&tree.root);
    assert_eq!(
        roles(&tree.root),
        vec![Some("AXApplication".to_owned()), Some("AXWindow".to_owned())]
    );
    assert!(root.menu_bar_excluded);
    assert!(!root.children_omitted, "an element the caller excluded is not an omitted child");
    assert!(tree.narrowed);
    assert!(!tree.truncated);
}

#[test]
fn a_nomination_that_matches_no_child_leaves_the_result_unnarrowed() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2, 3])
        .with(2, "AXMenuBar", &[])
        .with(3, "AXWindow", &[]);

    let tree = read_tree(&mut source, 1, excluding(99));

    assert_eq!(source.reads, vec![1, 2, 3]);
    assert!(!element_of(&tree.root).menu_bar_excluded);
    assert!(!tree.narrowed, "an element that was never there was not excluded");
    assert!(!tree.truncated);
}

#[test]
fn an_element_equal_to_the_nomination_is_still_read_below_the_root() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2])
        .with(2, "AXWindow", &[3])
        .with(3, "AXMenuBar", &[]);

    let tree = read_tree(&mut source, 1, excluding(3));

    assert_eq!(source.reads, vec![1, 2, 3], "only the root's own children are compared");
    assert_eq!(
        roles(&tree.root),
        vec![
            Some("AXApplication".to_owned()),
            Some("AXWindow".to_owned()),
            Some("AXMenuBar".to_owned()),
        ]
    );
    assert!(!element_of(&tree.root).menu_bar_excluded);
    assert!(!tree.narrowed);
}

#[test]
fn nothing_is_excluded_when_the_root_children_are_never_listed() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2, 3])
        .with(2, "AXMenuBar", &[])
        .with(3, "AXWindow", &[]);

    let tree =
        read_tree(&mut source, 1, TreeOptions { depth: Some(0), excluded_root_child: Some(2) });

    assert_eq!(source.reads, vec![1]);
    let root = element_of(&tree.root);
    assert!(root.depth_limited);
    assert!(!root.menu_bar_excluded, "a child that was never listed was not skipped");
    assert!(tree.narrowed);
}

#[test]
fn an_excluded_menu_and_depth_limited_descendants_mark_the_same_result() {
    let mut source = RecordedSource::default()
        .with(1, "AXApplication", &[2, 3])
        .with(2, "AXMenuBar", &[])
        .with(3, "AXWindow", &[4])
        .with(4, "AXButton", &[]);

    let tree =
        read_tree(&mut source, 1, TreeOptions { depth: Some(1), excluded_root_child: Some(2) });

    assert_eq!(source.reads, vec![1, 3]);
    let root = element_of(&tree.root);
    assert!(root.menu_bar_excluded);
    assert!(!root.depth_limited, "the root's own children were listed");
    assert!(element_of(&root.children[0]).depth_limited);
    assert!(tree.narrowed);
    assert!(!tree.truncated);
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
        self.expected_nodes_within(self.depth_limit)
    }

    /// The nodes of this tree down to `depth`, which is the whole tree when it is deeper.
    fn expected_nodes_within(&self, depth: usize) -> usize {
        let mut total = 0usize;
        let mut level = 1usize;
        for _ in 0..=self.depth_limit.min(depth) {
            total += level;
            level *= self.branching;
        }
        total
    }
}

impl ElementSource for GeneratedTree {
    type Element = u64;

    fn read(
        &mut self,
        element: &u64,
        expand_children: bool,
    ) -> Result<ElementAttributes<u64>, ReadFailure> {
        let depth = self.depth_of[element];
        let has_children = depth < self.depth_limit;
        let children = if !expand_children {
            ElementChildren::Unread { present: has_children }
        } else {
            let mut elements = Vec::new();
            if has_children {
                for _ in 0..self.branching {
                    let child = self.next_element;
                    self.next_element += 1;
                    self.depth_of.insert(child, depth + 1);
                    elements.push(child);
                }
            }
            ElementChildren::Read { elements, unreadable: false }
        };
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

fn is_narrowed_consistently(tree: &Tree) -> bool {
    tree.narrowed == leaves_out_what_was_asked_for(&tree.root)
}

proptest! {
    #[test]
    fn a_tree_that_fits_is_reported_whole(branching in 1usize..5, depth_limit in 0usize..6) {
        let mut source = GeneratedTree::new(branching, depth_limit);
        let expected = source.expected_nodes();
        prop_assume!(expected <= MAX_NODES);

        let tree = read_tree(&mut source, 0, TreeOptions::default());

        prop_assert_eq!(count_nodes(&tree.root), expected);
        prop_assert_eq!(deepest(&tree.root), depth_limit + 1);
        prop_assert!(!tree.truncated);
        prop_assert!(!tree.narrowed);
        prop_assert!(is_truncated_consistently(&tree));
        prop_assert!(is_narrowed_consistently(&tree));
    }
}

proptest! {
    // Each case builds 25,000 nodes, and the strategy has only three distinct inputs.
    #![proptest_config(ProptestConfig::with_cases(8))]

    #[test]
    fn a_tree_that_does_not_fit_stops_at_the_limit(branching in 2usize..5) {
        let mut source = GeneratedTree::new(branching, 1_000);

        let tree = read_tree(&mut source, 0, TreeOptions::default());

        prop_assert_eq!(count_nodes(&tree.root), MAX_NODES);
        prop_assert!(deepest(&tree.root) <= MAX_DEPTH + 1);
        prop_assert!(tree.truncated);
        prop_assert!(!tree.narrowed);
        prop_assert!(is_truncated_consistently(&tree));
        prop_assert!(is_narrowed_consistently(&tree));
    }
}

proptest! {
    #[test]
    fn a_requested_depth_bounds_the_walk_and_narrows_only_when_it_cuts(
        branching in 1usize..4,
        depth_limit in 0usize..5,
        requested in 0usize..6,
    ) {
        let mut source = GeneratedTree::new(branching, depth_limit);
        let expected = source.expected_nodes_within(requested);
        prop_assume!(expected <= MAX_NODES);

        let tree = read_tree(
            &mut source,
            0,
            TreeOptions { depth: Some(requested), excluded_root_child: None },
        );

        prop_assert_eq!(count_nodes(&tree.root), expected);
        prop_assert_eq!(deepest(&tree.root), depth_limit.min(requested) + 1);
        prop_assert_eq!(tree.narrowed, requested < depth_limit);
        prop_assert!(!tree.truncated);
        prop_assert!(is_truncated_consistently(&tree));
        prop_assert!(is_narrowed_consistently(&tree));
    }
}

proptest! {
    #[test]
    fn the_nominated_child_is_skipped_wherever_it_sits_among_the_root_children(
        children in 1usize..8,
        position in 0usize..8,
    ) {
        prop_assume!(position < children);
        let listed: Vec<u32> = (2..2 + children as u32).collect();
        let mut source = RecordedSource::default().with(1, "AXApplication", &listed);
        for child in &listed {
            source = source.with(*child, &format!("AXChild{child}"), &[]);
        }
        let nominated = listed[position];
        let kept: Vec<u32> = listed.iter().copied().filter(|child| *child != nominated).collect();

        let tree = read_tree(&mut source, 1, excluding(nominated));

        let mut expected_reads = vec![1];
        expected_reads.extend(kept.iter().copied());
        prop_assert_eq!(&source.reads, &expected_reads);
        let expected_roles: Vec<Option<String>> = std::iter::once(Some("AXApplication".to_owned()))
            .chain(kept.iter().map(|child| Some(format!("AXChild{child}"))))
            .collect();
        prop_assert_eq!(roles(&tree.root), expected_roles);
        let root = element_of(&tree.root);
        prop_assert!(root.menu_bar_excluded);
        prop_assert!(!root.children_omitted);
        prop_assert!(tree.narrowed);
        prop_assert!(!tree.truncated);
        prop_assert!(is_narrowed_consistently(&tree));
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

        let tree = read_tree(&mut source, 1, whole_tree());
        let node = element_of(&tree.root);
        let Some(AttributeValue::Text(reported)) = node.value.clone() else {
            return Err(TestCaseError::fail("a text value stays a text value"));
        };

        prop_assert!(text.starts_with(&reported));
        prop_assert_eq!(reported.chars().count(), scalars.min(MAX_VALUE_SCALARS));
        prop_assert_eq!(node.value_truncated, scalars > MAX_VALUE_SCALARS);
    }
}
