#![cfg(target_os = "macos")]
//! Proposal 0005: `ui correlate` reports evidence-backed candidate relationships between one
//! native observation and one external accessibility observation of the same window.
//!
//! Both reads stay separate evidence. A candidate records that the two sources reported matching
//! evidence; it never claims a native object and an accessibility element are one object, and no
//! identifier of one source is read as an identifier of the other. These tests pin the two
//! matching rules, what happens when the evidence is ambiguous or one-sided, and the independence
//! of each source's own observation, coverage, and coordinate transform.

#[path = "../src/macos/ui/correlate.rs"]
mod correlate;

use correlate::{Correlation, MatchBasis, ScreenTransform};
use serde_json::{Value, json};

/// The height of the fixture's primary display in logical points: AppKit's bottom-left global
/// origin and accessibility's top-left global origin are this far apart.
const PRIMARY_HEIGHT: f64 = 900.0;

/// One rectangle as the helper writes rectangles, saying which space and orientation it is in.
fn rect(orientation: &str, x: f64, y: f64, width: f64, height: f64) -> Value {
    json!({
        "status": "value",
        "space": "global-screen",
        "units": "logical-points",
        "orientation": orientation,
        "x": x, "y": y, "width": width, "height": height,
    })
}

/// One record as the helper's `basic` facet writes it, built field by field so each test shows
/// only the evidence it is about.
struct NativeNode(Value);

impl NativeNode {
    fn view(handle: &str, class: &str) -> Self {
        Self(json!({
            "handle": handle,
            "kind": "view",
            "depth": 1,
            "relationships": {},
            "basic": {"status": "value", "class": class, "superclasses": ["NSView"]},
        }))
    }

    fn window(handle: &str, class: &str) -> Self {
        Self(json!({
            "handle": handle,
            "kind": "window",
            "relationships": {},
            "basic": {"status": "value", "class": class, "superclasses": ["NSResponder"]},
        }))
    }

    fn basic(mut self, key: &str, value: Value) -> Self {
        self.0["basic"][key] = value;
        self
    }

    /// The in-process `accessibilityIdentifier` getter, which executes target code.
    fn accessibility_identifier(self, identifier: &str) -> Self {
        self.basic(
            "accessibilityIdentifier",
            json!({"status": "value", "value": identifier, "read": "target-executed"}),
        )
    }

    fn accessibility_role(self, role: &str) -> Self {
        self.basic(
            "accessibilityRole",
            json!({"status": "value", "value": role, "read": "target-executed"}),
        )
    }

    /// `NSView.identifier`, a different property with a different purpose.
    fn view_identifier(self, identifier: &str) -> Self {
        self.basic("identifier", json!({"status": "value", "value": identifier}))
    }

    fn flipped(self, flipped: bool) -> Self {
        self.basic("flipped", json!(flipped))
    }

    fn normalized(mut self, key: &str, value: Value) -> Self {
        if !self.0["basic"]["normalizedFrame"].is_object() {
            self.0["basic"]["normalizedFrame"] = json!({});
        }
        self.0["basic"]["normalizedFrame"][key] = value;
        self
    }

    fn top_left(self, x: f64, y: f64, width: f64, height: f64) -> Self {
        self.normalized("screenTopLeft", rect("top-left", x, y, width, height))
    }

    fn bottom_left(self, x: f64, y: f64, width: f64, height: f64) -> Self {
        self.normalized("screen", rect("bottom-left", x, y, width, height))
    }

    fn transform(self, display_id: u32, scale: f64) -> Self {
        self.normalized(
            "transform",
            json!({
                "status": "value",
                "method": "primary-screen-height-flip",
                "displayId": display_id,
                "scale": scale,
                "primaryScreenHeight": PRIMARY_HEIGHT,
                "orientation": "top-left",
            }),
        )
    }

    /// A view with everything correlation can compare: an accessibility role and a top-left
    /// global rectangle on the primary display.
    fn placed(handle: &str, role: &str, x: f64, y: f64, width: f64, height: f64) -> Value {
        Self::view(handle, "NSButton")
            .accessibility_role(role)
            .top_left(x, y, width, height)
            .bottom_left(x, PRIMARY_HEIGHT - y - height, width, height)
            .transform(1, 2.0)
            .value()
    }

    fn value(self) -> Value {
        self.0
    }
}

/// The helper's tree response, which carries its own observation interval and coverage.
fn native(nodes: Vec<Value>) -> Value {
    let returned = nodes.len();
    json!({
        "ok": true,
        "nodes": nodes,
        "observation": {
            "observationId": "n-8f21",
            "source": "native-appkit",
            "sessionId": "9d2f",
            "startTime": 1_761_600_000.25,
            "endTime": 1_761_600_000.75,
            "consistency": "main-thread-best-effort",
        },
        "coverage": {
            "visited": 24, "returned": returned,
            "narrowed": [], "truncated": [], "complete": true,
        },
    })
}

/// A window-scoped native tree, which returns the window it was scoped to beside the walk.
fn native_in_window(window: Value, nodes: Vec<Value>) -> Value {
    let mut response = native(nodes);
    response["windowRoots"] = json!([window]);
    response
}

/// One element of the external accessibility tree.
struct Element(Value);

impl Element {
    fn new(role: &str) -> Self {
        Self(json!({"role": role}))
    }

    fn identifier(mut self, identifier: &str) -> Self {
        self.0["identifier"] = json!(identifier);
        self
    }

    fn frame(mut self, x: f64, y: f64, width: f64, height: f64) -> Self {
        self.0["frame"] = json!({"x": x, "y": y, "width": width, "height": height});
        self
    }

    fn children(mut self, children: Vec<Value>) -> Self {
        self.0["children"] = Value::Array(children);
        self
    }

    fn value(self) -> Value {
        self.0
    }

    /// An element with a role and a global top-left frame, which is what accessibility reports.
    fn placed(role: &str, x: f64, y: f64, width: f64, height: f64) -> Value {
        Self::new(role).frame(x, y, width, height).value()
    }
}

/// What `recon ui ax tree` prints for one window.
fn accessibility(children: Vec<Value>) -> Value {
    json!({
        "application": {"path": "/Applications/Fixture.app", "pid": 412},
        "windowId": 53_510,
        "truncated": false,
        "narrowed": false,
        "root": Element::new("AXWindow").frame(0.0, 0.0, 800.0, 600.0).children(children).value(),
    })
}

/// Every candidate as `(native handle, accessibility path, basis, ambiguous)`.
fn candidates(correlation: &Correlation) -> Vec<(String, String, MatchBasis, bool)> {
    correlation
        .candidates
        .iter()
        .map(|candidate| {
            (
                candidate.native.handle.clone(),
                candidate.accessibility.path.clone(),
                candidate.basis,
                candidate.ambiguous,
            )
        })
        .collect()
}

fn unmatched_native(correlation: &Correlation) -> Vec<String> {
    correlation.unmatched_native.iter().map(|node| node.handle.clone()).collect()
}

fn unmatched_accessibility(correlation: &Correlation) -> Vec<String> {
    correlation.unmatched_accessibility.iter().map(|element| element.path.clone()).collect()
}

#[test]
fn an_accessibility_identifier_observed_on_both_sides_relates_two_observations() {
    let native = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSButton")
            .accessibility_identifier("send")
            .accessibility_role("AXButton")
            .top_left(10.0, 20.0, 80.0, 24.0)
            .value(),
    ]);
    let accessibility = accessibility(vec![
        Element::new("AXButton").identifier("send").frame(10.0, 20.0, 80.0, 24.0).value(),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(
        candidates(&correlation),
        vec![(
            "a1b2c3d4e5f60001".to_owned(),
            "AXWindow/AXButton[0]".to_owned(),
            MatchBasis::AccessibilityIdentifier,
            false,
        )]
    );
    let candidate = &correlation.candidates[0];
    assert_eq!(candidate.native.kind, "view");
    assert_eq!(candidate.native.class.as_deref(), Some("NSButton"));
    assert_eq!(candidate.accessibility.role.as_deref(), Some("AXButton"));
    assert_eq!(candidate.accessibility.identifier.as_deref(), Some("send"));
    assert_eq!(unmatched_native(&correlation), Vec::<String>::new());
    assert_eq!(unmatched_accessibility(&correlation), vec!["AXWindow".to_owned()]);
}

#[test]
fn a_native_view_identifier_is_never_read_as_an_accessibility_identifier() {
    let view_identifier = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSButton")
            .view_identifier("send")
            .accessibility_role("AXButton")
            .top_left(400.0, 400.0, 80.0, 24.0)
            .value(),
    ]);
    let accessibility = accessibility(vec![
        Element::new("AXButton").identifier("send").frame(10.0, 20.0, 80.0, 24.0).value(),
    ]);

    let correlation = correlate::correlate(&view_identifier, &accessibility);

    assert_eq!(candidates(&correlation), Vec::new());
    assert_eq!(unmatched_native(&correlation), vec!["a1b2c3d4e5f60001".to_owned()]);
    assert_eq!(
        unmatched_accessibility(&correlation),
        vec!["AXWindow".to_owned(), "AXWindow/AXButton[0]".to_owned()]
    );

    // The same records read through the property accessibility actually reports: what separates
    // the two runs is which native property was read, not the text or the geometry.
    let accessibility_identifier = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSButton")
            .accessibility_identifier("send")
            .accessibility_role("AXButton")
            .top_left(400.0, 400.0, 80.0, 24.0)
            .value(),
    ]);

    let correlation = correlate::correlate(&accessibility_identifier, &accessibility);

    assert_eq!(correlation.candidates.len(), 1);
    assert_eq!(correlation.candidates[0].basis, MatchBasis::AccessibilityIdentifier);
}

#[test]
fn duplicate_identifiers_keep_every_candidate_and_mark_each_ambiguous() {
    let native = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSTableRowView")
            .accessibility_identifier("row")
            .value(),
        NativeNode::view("a1b2c3d4e5f60002", "NSTableRowView")
            .accessibility_identifier("row")
            .value(),
    ]);
    let accessibility = accessibility(vec![
        Element::new("AXRow").identifier("row").value(),
        Element::new("AXRow").identifier("row").value(),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(
        candidates(&correlation),
        vec![
            (
                "a1b2c3d4e5f60001".to_owned(),
                "AXWindow/AXRow[0]".to_owned(),
                MatchBasis::AccessibilityIdentifier,
                true,
            ),
            (
                "a1b2c3d4e5f60001".to_owned(),
                "AXWindow/AXRow[1]".to_owned(),
                MatchBasis::AccessibilityIdentifier,
                true,
            ),
            (
                "a1b2c3d4e5f60002".to_owned(),
                "AXWindow/AXRow[0]".to_owned(),
                MatchBasis::AccessibilityIdentifier,
                true,
            ),
            (
                "a1b2c3d4e5f60002".to_owned(),
                "AXWindow/AXRow[1]".to_owned(),
                MatchBasis::AccessibilityIdentifier,
                true,
            ),
        ]
    );
    assert_eq!(unmatched_native(&correlation), Vec::<String>::new());
}

#[test]
fn an_empty_identifier_is_no_identifier_on_either_side() {
    let native = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSView")
            .accessibility_identifier("")
            .accessibility_role("AXGroup")
            .top_left(10.0, 20.0, 80.0, 24.0)
            .value(),
    ]);
    let accessibility = accessibility(vec![
        Element::new("AXGroup").identifier("").frame(10.0, 20.0, 80.0, 24.0).value(),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    // An empty identifier is absent evidence, so the pair falls to role and bounds instead of
    // matching every other element that reports no identifier either.
    assert_eq!(
        candidates(&correlation),
        vec![(
            "a1b2c3d4e5f60001".to_owned(),
            "AXWindow/AXGroup[0]".to_owned(),
            MatchBasis::RoleAndBounds,
            false,
        )]
    );
}

#[test]
fn a_role_and_bounds_match_allows_one_point_on_every_edge() {
    let native =
        native(vec![NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0)]);
    let accessibility = accessibility(vec![
        // Every edge moved by less than a point: left and top by 0.75, right and bottom by 0.25.
        Element::placed("AXButton", 10.75, 20.75, 79.5, 23.5),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(
        candidates(&correlation),
        vec![(
            "a1b2c3d4e5f60001".to_owned(),
            "AXWindow/AXButton[0]".to_owned(),
            MatchBasis::RoleAndBounds,
            false,
        )]
    );
}

#[test]
fn an_edge_further_than_one_point_away_is_not_a_match() {
    let native =
        native(vec![NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0)]);
    let accessibility = accessibility(vec![
        // Origin and size are each within a point of the native record, but the right edge is
        // 1.5 points away, so this is not the same rectangle.
        Element::placed("AXButton", 10.0, 20.0, 81.5, 24.0),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(candidates(&correlation), Vec::new());
    assert_eq!(unmatched_native(&correlation), vec!["a1b2c3d4e5f60001".to_owned()]);
}

#[test]
fn a_different_role_is_not_a_bounds_match() {
    let native =
        native(vec![NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0)]);
    let accessibility = accessibility(vec![Element::placed("AXCheckBox", 10.0, 20.0, 80.0, 24.0)]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(candidates(&correlation), Vec::new());
}

#[test]
fn two_different_accessibility_identifiers_are_evidence_against_a_relationship() {
    let native = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSButton")
            .accessibility_identifier("send")
            .accessibility_role("AXButton")
            .top_left(10.0, 20.0, 80.0, 24.0)
            .value(),
    ]);
    let accessibility = accessibility(vec![
        Element::new("AXButton").identifier("cancel").frame(10.0, 20.0, 80.0, 24.0).value(),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(candidates(&correlation), Vec::new());
    assert_eq!(unmatched_native(&correlation), vec!["a1b2c3d4e5f60001".to_owned()]);
}

#[test]
fn equally_placed_elements_keep_every_candidate() {
    let native =
        native(vec![NativeNode::placed("a1b2c3d4e5f60001", "AXGroup", 10.0, 20.0, 80.0, 24.0)]);
    let accessibility = accessibility(vec![
        Element::placed("AXGroup", 10.0, 20.0, 80.0, 24.0),
        Element::placed("AXGroup", 10.5, 20.0, 80.0, 24.0),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(
        candidates(&correlation),
        vec![
            (
                "a1b2c3d4e5f60001".to_owned(),
                "AXWindow/AXGroup[0]".to_owned(),
                MatchBasis::RoleAndBounds,
                true,
            ),
            (
                "a1b2c3d4e5f60001".to_owned(),
                "AXWindow/AXGroup[1]".to_owned(),
                MatchBasis::RoleAndBounds,
                true,
            ),
        ]
    );
    assert_eq!(unmatched_accessibility(&correlation), vec!["AXWindow".to_owned()]);
}

#[test]
fn a_native_object_accessibility_does_not_expose_is_unmatched() {
    let native = native(vec![
        NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0),
        // A content view and a cell carrier that the accessibility model is free to omit.
        NativeNode::view("a1b2c3d4e5f60002", "NSView")
            .accessibility_role("AXUnknown")
            .top_left(0.0, 0.0, 800.0, 600.0)
            .value(),
    ]);
    let accessibility = accessibility(vec![Element::placed("AXButton", 10.0, 20.0, 80.0, 24.0)]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(correlation.candidates.len(), 1);
    assert_eq!(unmatched_native(&correlation), vec!["a1b2c3d4e5f60002".to_owned()]);
    assert_eq!(correlation.native.observed, 2);
    assert_eq!(correlation.accessibility.observed, 2);
}

#[test]
fn an_accessibility_element_with_no_native_record_is_unmatched() {
    let native =
        native(vec![NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0)]);
    let accessibility = accessibility(vec![
        Element::placed("AXButton", 10.0, 20.0, 80.0, 24.0),
        Element::placed("AXStaticText", 10.0, 60.0, 200.0, 16.0),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(correlation.candidates.len(), 1);
    assert_eq!(
        unmatched_accessibility(&correlation),
        vec!["AXWindow".to_owned(), "AXWindow/AXStaticText[1]".to_owned()]
    );
}

#[test]
fn each_source_keeps_its_own_observation_coverage_and_transform() {
    let mut native =
        native(vec![NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0)]);
    native["observation"]["startTime"] = json!(1_761_600_010.0);
    native["observation"]["endTime"] = json!(1_761_600_010.5);
    native["coverage"]["truncated"] = json!(["returned:limit"]);
    let mut accessibility =
        accessibility(vec![Element::placed("AXButton", 10.0, 20.0, 80.0, 24.0)]);
    accessibility["truncated"] = json!(true);
    accessibility["narrowed"] = json!(true);

    let correlation = correlate::correlate(&native, &accessibility);

    // The native observation is kept exactly as the helper reported it: a correlation relates two
    // readings taken at different times and never merges their intervals.
    assert_eq!(correlation.native.observation.as_ref(), Some(&native["observation"]));
    assert_eq!(correlation.native.coverage.as_ref(), Some(&native["coverage"]));
    assert_eq!(
        correlation.native.observation.as_ref().unwrap()["startTime"],
        json!(1_761_600_010.0)
    );
    assert!(correlation.accessibility.truncated);
    assert!(correlation.accessibility.narrowed);
    assert_eq!(
        correlation.native.transforms,
        vec![ScreenTransform {
            display_id: Some(1),
            scale: Some(2.0),
            primary_screen_height: Some(PRIMARY_HEIGHT),
        }]
    );
    assert_eq!(correlation.candidates.len(), 1);
}

#[test]
fn a_flipped_view_is_compared_through_its_top_left_normalization() {
    let native = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSTextField")
            .flipped(true)
            .accessibility_role("AXStaticText")
            .bottom_left(100.0, 500.0, 40.0, 20.0)
            .top_left(100.0, PRIMARY_HEIGHT - 520.0, 40.0, 20.0)
            .transform(1, 1.0)
            .value(),
    ]);
    let accessibility = accessibility(vec![
        // The element the native record actually describes, in accessibility's top-left space.
        Element::placed("AXStaticText", 100.0, 380.0, 40.0, 20.0),
        // An element that would match only if the bottom-left numbers were read as top-left ones.
        Element::placed("AXStaticText", 100.0, 500.0, 40.0, 20.0),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(
        candidates(&correlation),
        vec![(
            "a1b2c3d4e5f60001".to_owned(),
            "AXWindow/AXStaticText[0]".to_owned(),
            MatchBasis::RoleAndBounds,
            false,
        )]
    );
    assert_eq!(
        unmatched_accessibility(&correlation),
        vec!["AXWindow".to_owned(), "AXWindow/AXStaticText[1]".to_owned()]
    );
}

#[test]
fn bottom_left_bounds_alone_are_never_compared_as_top_left_bounds() {
    let native = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSTextField")
            .accessibility_role("AXStaticText")
            .bottom_left(100.0, 500.0, 40.0, 20.0)
            .value(),
    ]);
    let accessibility =
        accessibility(vec![Element::placed("AXStaticText", 100.0, 500.0, 40.0, 20.0)]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(candidates(&correlation), Vec::new());
    assert_eq!(unmatched_native(&correlation), vec!["a1b2c3d4e5f60001".to_owned()]);
}

#[test]
fn negative_coordinates_from_a_second_display_are_compared_unchanged() {
    let native = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSButton")
            .accessibility_role("AXButton")
            // A display left of and above the primary one: both normalized coordinates are
            // negative, which is a position, not an error.
            .top_left(-1_520.0, -180.0, 80.0, 24.0)
            .bottom_left(-1_520.0, PRIMARY_HEIGHT + 156.0, 80.0, 24.0)
            .transform(69_733_382, 2.0)
            .value(),
    ]);
    let accessibility =
        accessibility(vec![Element::placed("AXButton", -1_520.0, -180.0, 80.0, 24.0)]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(correlation.candidates.len(), 1);
    assert_eq!(correlation.candidates[0].basis, MatchBasis::RoleAndBounds);
    assert_eq!(correlation.native.transforms[0].display_id, Some(69_733_382));
}

#[test]
fn a_non_unit_backing_scale_never_scales_logical_points() {
    let native = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSButton")
            .accessibility_role("AXButton")
            .top_left(100.0, 380.0, 40.0, 20.0)
            .transform(1, 2.0)
            .value(),
    ]);
    let accessibility = accessibility(vec![
        Element::placed("AXButton", 100.0, 380.0, 40.0, 20.0),
        // The same rectangle in backing pixels, which no scale is applied to reach.
        Element::placed("AXButton", 200.0, 760.0, 80.0, 40.0),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(
        candidates(&correlation),
        vec![(
            "a1b2c3d4e5f60001".to_owned(),
            "AXWindow/AXButton[0]".to_owned(),
            MatchBasis::RoleAndBounds,
            false,
        )]
    );
    assert_eq!(correlation.native.transforms[0].scale, Some(2.0));
}

#[test]
fn bounds_in_another_unit_or_space_are_not_reinterpreted() {
    let mut node = NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0);
    node["basic"]["normalizedFrame"]["screenTopLeft"]["units"] = json!("backing-pixels");
    let native = native(vec![node]);
    let accessibility = accessibility(vec![Element::placed("AXButton", 10.0, 20.0, 80.0, 24.0)]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(candidates(&correlation), Vec::new());
}

#[test]
fn an_unobserved_normalization_leaves_a_node_without_comparable_bounds() {
    let native = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSButton")
            .accessibility_role("AXButton")
            .basic("normalizedFrame", json!({"status": "absent"}))
            .value(),
    ]);
    let accessibility = accessibility(vec![Element::placed("AXButton", 10.0, 20.0, 80.0, 24.0)]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(candidates(&correlation), Vec::new());
    assert_eq!(unmatched_native(&correlation), vec!["a1b2c3d4e5f60001".to_owned()]);
}

#[test]
fn a_repeated_native_reference_is_not_a_second_object() {
    let mut native = native(vec![
        NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0),
        json!({"handle": "a1b2c3d4e5f60001", "kind": "view", "depth": 2, "reference": true}),
    ]);
    native["coverage"]["returned"] = json!(2);

    let correlation = correlate::correlate(&native, &accessibility(vec![]));

    assert_eq!(correlation.native.observed, 1);
    assert_eq!(unmatched_native(&correlation), vec!["a1b2c3d4e5f60001".to_owned()]);
}

#[test]
fn repeated_and_failed_accessibility_nodes_are_not_subjects() {
    let native =
        native(vec![NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0)]);
    let accessibility = accessibility(vec![
        json!({"role": "AXButton", "repeated": true}),
        json!({"error": "element-unavailable"}),
        Element::placed("AXButton", 10.0, 20.0, 80.0, 24.0),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    // The repeated element is the one already read, and the failed one was never read; neither is
    // a separate observation to relate. Their positions still name the element that follows them.
    assert_eq!(
        candidates(&correlation),
        vec![(
            "a1b2c3d4e5f60001".to_owned(),
            "AXWindow/AXButton[2]".to_owned(),
            MatchBasis::RoleAndBounds,
            false,
        )]
    );
    assert_eq!(correlation.accessibility.observed, 2);
}

#[test]
fn a_native_record_without_a_handle_and_kind_is_not_returned_as_a_reference() {
    let native = native(vec![
        json!({"kind": "view", "basic": {"status": "value", "class": "NSView"}}),
        NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0),
    ]);

    let correlation = correlate::correlate(&native, &accessibility(vec![]));

    assert_eq!(unmatched_native(&correlation), vec!["a1b2c3d4e5f60001".to_owned()]);
    assert_eq!(correlation.native.observed, 1);
}

#[test]
fn an_accessibility_path_describes_a_position_in_this_observation() {
    let accessibility = accessibility(vec![
        Element::new("AXGroup")
            .children(vec![Element::new("AXButton").identifier("send").value()])
            .value(),
    ]);
    let native = native(vec![
        NativeNode::view("a1b2c3d4e5f60001", "NSButton").accessibility_identifier("send").value(),
    ]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(correlation.candidates[0].accessibility.path, "AXWindow/AXGroup[0]/AXButton[0]");
}

#[test]
fn an_absent_source_correlates_nothing_rather_than_guessing() {
    let correlation = correlate::correlate(&json!({"ok": true}), &json!({}));

    assert_eq!(correlation.candidates.len(), 0);
    assert_eq!(correlation.native.observed, 0);
    assert_eq!(correlation.accessibility.observed, 0);
    assert_eq!(correlation.native.observation, None);
    assert_eq!(correlation.native.coverage, None);
}

#[test]
fn the_window_a_native_walk_was_scoped_to_is_one_of_its_records() {
    let native = native_in_window(
        // A window record's own frame is already global and bottom-left, so its normalization is
        // the top-left rectangle alone.
        NativeNode::window("a1b2c3d4e5f60000", "NSWindow")
            .accessibility_role("AXWindow")
            .basic("frame", rect("bottom-left", 0.0, PRIMARY_HEIGHT - 600.0, 800.0, 600.0))
            .top_left(0.0, 0.0, 800.0, 600.0)
            .transform(1, 2.0)
            .value(),
        vec![NativeNode::placed("a1b2c3d4e5f60001", "AXButton", 10.0, 20.0, 80.0, 24.0)],
    );
    let accessibility = accessibility(vec![Element::placed("AXButton", 10.0, 20.0, 80.0, 24.0)]);

    let correlation = correlate::correlate(&native, &accessibility);

    assert_eq!(
        candidates(&correlation),
        vec![
            (
                "a1b2c3d4e5f60000".to_owned(),
                "AXWindow".to_owned(),
                MatchBasis::RoleAndBounds,
                false,
            ),
            (
                "a1b2c3d4e5f60001".to_owned(),
                "AXWindow/AXButton[0]".to_owned(),
                MatchBasis::RoleAndBounds,
                false,
            ),
        ]
    );
    assert_eq!(correlation.candidates[0].native.kind, "window");
    assert_eq!(correlation.native.observed, 2);
    assert_eq!(unmatched_accessibility(&correlation), Vec::<String>::new());
}
