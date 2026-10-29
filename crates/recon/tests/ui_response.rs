use std::path::Path;

use proptest::prelude::*;
use recon::application::Pid;
use recon::cli::schema::output_schema;
use recon::ui::response::{
    CapturedImage, CapturedWindow, ObservedApplication, ScreenshotResponse, TreeResponse,
    WindowsResponse,
};
use recon::ui::tree::{AttributeValue, ElementNode, Node, ReadFailure};
use recon::ui::window::{Bounds, Window, WindowId};
use serde_json::{Value, json};

const BUNDLE: &str = "/Applications/Notes.app";

fn application() -> ObservedApplication<'static> {
    ObservedApplication { path: Path::new(BUNDLE), pid: Pid(412) }
}

fn bounds() -> Bounds {
    Bounds { x: 10.0, y: 20.0, width: 300.5, height: 200.0 }
}

fn window(title: Option<&str>) -> Window {
    Window {
        id: WindowId(53510),
        owner_pid: Pid(412),
        title: title.map(str::to_owned),
        layer: 0,
        bounds: bounds(),
        on_screen: true,
        alpha: 1.0,
    }
}

/// An element that answered and exposed nothing.
fn node() -> ElementNode {
    ElementNode {
        role: None,
        subrole: None,
        title: None,
        description: None,
        value: None,
        value_truncated: false,
        identifier: None,
        enabled: None,
        focused: None,
        frame: None,
        actions: Vec::new(),
        children: Vec::new(),
        children_omitted: false,
        depth_limited: false,
        menu_bar_excluded: false,
    }
}

fn with_role(role: &str) -> ElementNode {
    ElementNode { role: Some(role.to_owned()), ..node() }
}

fn encoded(value: &impl serde::Serialize) -> Value {
    serde_json::to_value(value).expect("serialize")
}

/// The field names of a JSON object, sorted so a test states which fields are there rather than
/// the order they happen to be written in.
fn fields(value: &Value) -> Vec<&str> {
    let mut names: Vec<&str> =
        value.as_object().expect("an object").keys().map(String::as_str).collect();
    names.sort_unstable();
    names
}

#[test]
fn a_window_without_a_title_reports_null() {
    let encoded = encoded(&WindowsResponse::new(application(), vec![window(None)]));
    assert!(encoded["windows"][0].get("title").is_some());
    assert_eq!(encoded["windows"][0]["title"], Value::Null);
}

#[test]
fn the_owning_process_is_not_part_of_a_window() {
    let encoded = encoded(&window(None));
    assert!(encoded.get("ownerPid").is_none());
    assert!(encoded.get("owner_pid").is_none());
}

#[test]
fn a_node_reports_every_attribute_the_element_has() {
    let mut element = with_role("AXButton");
    element.title = Some("Record".to_owned());
    element.value = Some(AttributeValue::Text("fixture value".to_owned()));
    element.enabled = Some(true);
    element.focused = Some(false);
    element.frame = Some(bounds());
    element.actions = vec!["AXPress".to_owned()];

    let encoded = encoded(&Node::Element(Box::new(element)));

    assert_eq!(
        fields(&encoded),
        ["actions", "enabled", "focused", "frame", "role", "title", "value"]
    );
    assert_eq!(encoded["role"], json!("AXButton"));
    assert_eq!(encoded["title"], json!("Record"));
    assert_eq!(encoded["value"], json!("fixture value"));
    assert_eq!(encoded["enabled"], json!(true));
    assert_eq!(encoded["focused"], json!(false));
    assert_eq!(encoded["frame"], json!({"x": 10.0, "y": 20.0, "width": 300.5, "height": 200.0}));
    assert_eq!(encoded["actions"], json!(["AXPress"]));
}

#[test]
fn a_node_omits_every_attribute_the_element_does_not_have() {
    assert_eq!(fields(&encoded(&Node::Element(Box::new(with_role("AXGroup"))))), ["role"]);
}

#[test]
fn an_element_that_exposes_nothing_is_an_empty_object() {
    assert_eq!(encoded(&Node::Element(Box::new(node()))), json!({}));
}

#[test]
fn an_attribute_that_is_present_but_empty_or_false_is_still_reported() {
    let mut element = with_role("AXCheckBox");
    element.title = Some(String::new());
    element.value = Some(AttributeValue::Integer(0));
    element.enabled = Some(false);
    element.focused = Some(false);
    element.frame = Some(Bounds { x: 0.0, y: 0.0, width: 0.0, height: 0.0 });

    let encoded = encoded(&Node::Element(Box::new(element)));

    assert_eq!(fields(&encoded), ["enabled", "focused", "frame", "role", "title", "value"]);
    assert_eq!(encoded["title"], json!(""));
    assert_eq!(encoded["value"], json!(0));
    assert_eq!(encoded["enabled"], json!(false));
    assert_eq!(encoded["focused"], json!(false));
    assert_eq!(encoded["frame"], json!({"x": 0.0, "y": 0.0, "width": 0.0, "height": 0.0}));
}

#[test]
fn a_node_reports_children_only_when_it_has_them() {
    let mut element = with_role("AXWindow");
    element.children = vec![Node::Element(Box::new(with_role("AXStaticText")))];

    let encoded = encoded(&Node::Element(Box::new(element)));

    assert_eq!(fields(&encoded), ["children", "role"]);
    assert_eq!(encoded["children"], json!([{"role": "AXStaticText"}]));
}

#[test]
fn every_kind_of_value_keeps_its_json_type() {
    let kinds = [
        (AttributeValue::Text("text".to_owned()), json!("text")),
        (AttributeValue::Integer(-7), json!(-7)),
        (AttributeValue::Real(1.5), json!(1.5)),
        (AttributeValue::Boolean(true), json!(true)),
    ];

    for (value, expected) in kinds {
        let mut element = with_role("AXValueHolder");
        element.value = Some(value);

        assert_eq!(encoded(&Node::Element(Box::new(element)))["value"], expected);
    }
}

#[test]
fn non_finite_values_are_absent_rather_than_null_in_sparse_nodes() {
    let schema = output_schema::<Node>();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let element = ElementNode { value: Some(AttributeValue::Real(number)), ..node() };
        let encoded = encoded(&Node::Element(Box::new(element)));
        assert!(encoded.get("value").is_none(), "{encoded}");
        assert!(validator.is_valid(&encoded), "{encoded}");
    }
}

#[test]
fn non_finite_frame_components_omit_only_the_atomic_frame() {
    let schema = output_schema::<TreeResponse<'static>>();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    let observe = |[x, y, width, height]: [f64; 4]| {
        encoded(&TreeResponse::new(
            application(),
            Some(WindowId(53510)),
            false,
            false,
            Node::Element(Box::new(ElementNode {
                title: Some("Frame holder".to_owned()),
                value: Some(AttributeValue::Boolean(false)),
                actions: vec!["AXRaise".to_owned()],
                frame: Some(Bounds { x, y, width, height }),
                ..with_role("AXWindow")
            })),
        ))
    };
    let mut expected = observe([1.0, 2.0, 3.0, 4.0]);
    expected["root"].as_object_mut().unwrap().remove("frame");
    for component in 0..4 {
        for number in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut coordinates = [1.0, 2.0, 3.0, 4.0];
            coordinates[component] = number;
            let observed = observe(coordinates);
            assert_eq!(observed, expected);
            assert!(validator.is_valid(&observed));
        }
    }
}

#[test]
fn finite_frames_keep_every_component_unchanged() {
    let schema = output_schema::<Node>();
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for coordinates in [
        [0.0, -0.0, 0.0, -0.0],
        [-1.0, 2.0, -3.0, 4.0],
        [f64::MIN, f64::MAX, f64::MIN_POSITIVE, f64::from_bits(1)],
    ] {
        let [x, y, width, height] = coordinates;
        let element = ElementNode { frame: Some(Bounds { x, y, width, height }), ..node() };
        let observed = encoded(&Node::Element(Box::new(element)));
        for (field, expected) in ["x", "y", "width", "height"].into_iter().zip(coordinates) {
            assert_eq!(observed["frame"][field].as_f64().unwrap().to_bits(), expected.to_bits());
        }
        assert!(validator.is_valid(&observed));
    }
}

#[test]
fn a_repeated_node_carries_its_role_only_when_it_had_one() {
    assert_eq!(
        encoded(&Node::Repeated { role: Some("AXWindow".to_owned()) }),
        json!({"role": "AXWindow", "repeated": true})
    );
    assert_eq!(encoded(&Node::Repeated { role: None }), json!({"repeated": true}));
}

#[test]
fn a_failed_node_names_its_failure() {
    assert_eq!(
        encoded(&Node::Failed(ReadFailure::Unavailable)),
        json!({"error": "element-unavailable"})
    );
    assert_eq!(encoded(&Node::Failed(ReadFailure::Timeout)), json!({"error": "timeout"}));
    assert_eq!(
        encoded(&Node::Failed(ReadFailure::Failed(-25_200))),
        json!({"error": "read-failed", "axError": -25200})
    );
}

#[test]
fn the_tree_response_names_the_observation_it_describes() {
    let response = TreeResponse::new(
        application(),
        Some(WindowId(53510)),
        true,
        false,
        Node::Failed(ReadFailure::Timeout),
    );

    let encoded = encoded(&response);

    assert_eq!(fields(&encoded), ["application", "narrowed", "root", "truncated", "windowId"]);
    assert_eq!(encoded["application"], json!({"path": BUNDLE, "pid": 412}));
    assert_eq!(encoded["windowId"], json!(53510));
    assert_eq!(encoded["root"], json!({"error": "timeout"}));
}

#[test]
fn the_tree_envelope_keeps_every_field_whatever_the_result() {
    for (truncated, narrowed) in [(false, false), (true, false), (false, true), (true, true)] {
        let response = TreeResponse::new(
            application(),
            None,
            truncated,
            narrowed,
            Node::Element(Box::new(node())),
        );

        let encoded = encoded(&response);

        assert_eq!(fields(&encoded), ["application", "narrowed", "root", "truncated", "windowId"]);
        assert_eq!(encoded["windowId"], Value::Null, "an application tree names no window");
        assert_eq!(encoded["truncated"], json!(truncated));
        assert_eq!(encoded["narrowed"], json!(narrowed));
        assert_eq!(encoded["root"], json!({}));
    }
}

#[test]
fn the_screenshot_response_names_the_capture_method() {
    let response = ScreenshotResponse::new(
        application(),
        CapturedWindow {
            id: WindowId(53510),
            title: Some("Notes"),
            bounds: bounds(),
            on_screen: true,
        },
        CapturedImage { path: Path::new("/tmp/shot.png"), width: 600, height: 400, uniform: false },
    );

    assert_eq!(encoded(&response)["method"], "skylight-hardware-capture");
}

proptest! {
    #[test]
    fn sparse_nodes_preserve_present_attributes_and_never_emit_absence(
        text in any::<[Option<String>; 5]>(),
        flags in any::<[Option<bool>; 2]>(),
        value in any::<Option<i64>>(),
        frame in any::<Option<[i16; 4]>>(),
        actions in prop::collection::vec(any::<String>(), 0..4),
        has_child in any::<bool>(),
        markers in any::<[bool; 4]>(),
    ) {
        let element = ElementNode {
            role: text[0].clone(),
            subrole: text[1].clone(),
            title: text[2].clone(),
            description: text[3].clone(),
            identifier: text[4].clone(),
            enabled: flags[0],
            focused: flags[1],
            value: value.map(AttributeValue::Integer),
            frame: frame.map(|[x, y, width, height]| Bounds {
                x: f64::from(x), y: f64::from(y),
                width: f64::from(width), height: f64::from(height),
            }),
            actions: actions.clone(),
            children: if has_child { vec![Node::Repeated { role: None }] } else { Vec::new() },
            value_truncated: markers[0],
            children_omitted: markers[1],
            depth_limited: markers[2],
            menu_bar_excluded: markers[3],
        };
        let encoded = encoded(&Node::Element(Box::new(element)));
        for field in encoded.as_object().unwrap().values() {
            prop_assert!(!field.is_null());
            prop_assert!(!field.as_array().is_some_and(Vec::is_empty));
        }
        for (name, expected) in ["role", "subrole", "title", "description", "identifier"].into_iter().zip(text) {
            prop_assert_eq!(encoded.get(name).cloned(), expected.map(Value::String));
        }
        for (name, expected) in ["enabled", "focused"].into_iter().zip(flags) {
            prop_assert_eq!(encoded.get(name).cloned(), expected.map(Value::Bool));
        }
        prop_assert_eq!(encoded.get("value").cloned(), value.map(|number| json!(number)));
        prop_assert_eq!(encoded.get("frame").is_some(), frame.is_some());
        prop_assert_eq!(encoded.get("actions").cloned(), (!actions.is_empty()).then(|| json!(actions)));
        prop_assert_eq!(encoded.get("children").is_some(), has_child);
        for (name, expected) in ["valueTruncated", "childrenOmitted", "depthLimited", "menuBarExcluded"].into_iter().zip(markers) {
            prop_assert_eq!(encoded.get(name).cloned(), expected.then_some(Value::Bool(true)));
        }
    }
}
