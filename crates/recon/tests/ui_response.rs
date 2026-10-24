use std::path::Path;

use recon::application::Pid;
use recon::ui::response::{
    CapturedImage, CapturedWindow, ObservedApplication, ScreenshotResponse, TreeResponse,
    WindowsResponse,
};
use recon::ui::tree::{AttributeValue, ElementNode, Node, ReadFailure};
use recon::ui::window::{Bounds, Window, WindowId};

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

fn node(role: &str) -> ElementNode {
    ElementNode {
        role: Some(role.to_owned()),
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
    }
}

fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("serialize")
}

#[test]
fn the_windows_response_matches_the_proposal() {
    let response =
        WindowsResponse { application: application(), windows: vec![window(Some("Notes"))] };

    assert_eq!(
        json(&response),
        r#"{"application":{"path":"/Applications/Notes.app","pid":412},"windows":[{"id":53510,"title":"Notes","layer":0,"bounds":{"x":10.0,"y":20.0,"width":300.5,"height":200.0},"onScreen":true,"alpha":1.0}]}"#
    );
}

#[test]
fn a_window_without_a_title_reports_null() {
    let encoded =
        json(&WindowsResponse { application: application(), windows: vec![window(None)] });
    assert!(encoded.contains(r#""id":53510,"title":null,"layer":0"#), "{encoded}");
}

#[test]
fn the_owning_process_is_not_part_of_a_window() {
    assert!(!json(&window(None)).contains("ownerPid"));
    assert!(!json(&window(None)).contains("owner_pid"));
}

#[test]
fn an_element_node_omits_the_optional_flags_when_false() {
    let mut element = node("AXButton");
    element.title = Some("Record".to_owned());
    element.value = Some(AttributeValue::Text("fixture value".to_owned()));
    element.enabled = Some(true);
    element.focused = Some(false);
    element.frame = Some(bounds());
    element.actions = vec!["AXPress".to_owned()];

    assert_eq!(
        json(&Node::Element(Box::new(element))),
        r#"{"role":"AXButton","subrole":null,"title":"Record","description":null,"value":"fixture value","identifier":null,"enabled":true,"focused":false,"frame":{"x":10.0,"y":20.0,"width":300.5,"height":200.0},"actions":["AXPress"],"children":[]}"#
    );
}

#[test]
fn a_cut_value_and_omitted_children_are_flagged() {
    let mut element = node("AXTextArea");
    element.value = Some(AttributeValue::Text("cut".to_owned()));
    element.value_truncated = true;
    element.children = vec![Node::Element(Box::new(node("AXStaticText")))];
    element.children_omitted = true;

    let encoded = json(&Node::Element(Box::new(element)));
    let cut = r#""value":"cut","valueTruncated":true,"identifier":null"#;
    let child = r#""children":[{"role":"AXStaticText","subrole":null,"title":null,"description":null,"value":null,"identifier":null,"enabled":null,"focused":null,"frame":null,"actions":[],"children":[]}],"childrenOmitted":true}"#;
    assert!(encoded.contains(cut), "{encoded}");
    assert!(encoded.ends_with(child), "{encoded}");
}

#[test]
fn every_kind_of_value_keeps_its_json_type() {
    let kinds = [
        (AttributeValue::Text("text".to_owned()), r#""text""#),
        (AttributeValue::Integer(-7), "-7"),
        (AttributeValue::Real(1.5), "1.5"),
        (AttributeValue::Boolean(true), "true"),
    ];
    for (value, expected) in kinds {
        let mut element = node("AXValueHolder");
        element.value = Some(value);
        let encoded = json(&Node::Element(Box::new(element)));
        let expected_value = format!(r#""value":{expected},"identifier""#);
        assert!(encoded.contains(&expected_value), "{encoded}");
    }
}

#[test]
fn a_repeated_node_carries_only_its_role() {
    assert_eq!(
        json(&Node::Repeated { role: Some("AXWindow".to_owned()) }),
        r#"{"role":"AXWindow","repeated":true}"#
    );
    assert_eq!(json(&Node::Repeated { role: None }), r#"{"role":null,"repeated":true}"#);
}

#[test]
fn a_failed_node_names_its_failure() {
    assert_eq!(json(&Node::Failed(ReadFailure::Unavailable)), r#"{"error":"element-unavailable"}"#);
    assert_eq!(json(&Node::Failed(ReadFailure::Timeout)), r#"{"error":"timeout"}"#);
    assert_eq!(
        json(&Node::Failed(ReadFailure::Failed(-25_200))),
        r#"{"error":"read-failed","axError":-25200}"#
    );
}

#[test]
fn the_tree_response_matches_the_proposal() {
    let response = TreeResponse {
        application: application(),
        window_id: Some(WindowId(53510)),
        truncated: true,
        root: Node::Failed(ReadFailure::Timeout),
    };

    assert_eq!(
        json(&response),
        r#"{"application":{"path":"/Applications/Notes.app","pid":412},"windowId":53510,"truncated":true,"root":{"error":"timeout"}}"#
    );

    let whole = TreeResponse {
        application: application(),
        window_id: None,
        truncated: false,
        root: Node::Repeated { role: None },
    };
    assert!(json(&whole).contains(r#""windowId":null,"truncated":false"#), "{}", json(&whole));
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

    assert_eq!(
        json(&response),
        r#"{"application":{"path":"/Applications/Notes.app","pid":412},"window":{"id":53510,"title":"Notes","bounds":{"x":10.0,"y":20.0,"width":300.5,"height":200.0},"onScreen":true},"image":{"path":"/tmp/shot.png","width":600,"height":400,"uniform":false},"method":"skylight-hardware-capture"}"#
    );
}
