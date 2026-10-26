use super::harness::{
    Fixture, INSPECTOR_WINDOW_TITLE, MAIN_WINDOW_TITLE, main_window_id, nodes, run_ui,
};
use serde_json::Value;

fn children(node: &Value) -> &[Value] {
    node["children"].as_array().map_or(&[], Vec::as_slice)
}

fn control_focus(root: &Value) -> [bool; 3] {
    let elements = nodes(root);
    let focused = |role: &str, field: &str, value: &str| {
        elements
            .iter()
            .find(|node| node["role"] == role && node[field] == value)
            .expect("known fixture control")["focused"]
            .as_bool()
            .expect("fixture control exposes focus")
    };
    [
        focused("AXButton", "title", "Record"),
        focused("AXCheckBox", "title", "Enabled"),
        focused("AXTextField", "value", "fixture value"),
    ]
}

#[test]
#[ignore = "needs signed recon with Accessibility granted and the built fixture; run vp run test:ui"]
fn proposal_0004_menu_exclusion_keeps_windows_and_status_items_without_changing_focus_or_pointer() {
    let fixture = Fixture::launch();
    let full = run_ui(&["tree", fixture.target()]);
    let narrowed = run_ui(&["tree", fixture.target(), "--no-menu-bar"]);
    assert_eq!(control_focus(&full["root"]), control_focus(&narrowed["root"]));

    assert_eq!(full["narrowed"], false);
    assert_eq!(full["truncated"], false);
    assert!(nodes(&full["root"]).iter().any(|node| node["title"] == "Fixture"));
    assert!(!nodes(&narrowed["root"]).iter().any(|node| node["title"] == "Fixture"));
    assert_eq!(narrowed["root"]["menuBarExcluded"], true);
    assert_eq!(narrowed["narrowed"], true);
    assert_eq!(narrowed["truncated"], false);
    for title in [MAIN_WINDOW_TITLE, INSPECTOR_WINDOW_TITLE] {
        assert!(
            nodes(&narrowed["root"])
                .iter()
                .any(|node| node["role"] == "AXWindow" && node["title"] == title),
            "missing {title}: {narrowed}"
        );
    }
    for tree in [&full, &narrowed] {
        assert!(
            nodes(&tree["root"]).iter().any(|node| node["subrole"] == "AXMenuExtra"),
            "status-item menu missing: {tree}"
        );
    }
    assert!(
        serde_json::to_vec(&narrowed).unwrap().len() < serde_json::to_vec(&full).unwrap().len()
    );
}

#[test]
#[ignore = "needs signed recon with Accessibility granted and the built fixture; run vp run test:ui"]
fn proposal_0004_root_zero_and_depth_one_preserve_distinct_omission_markers() {
    let fixture = Fixture::launch();
    for suffix in [vec!["--depth", "0"], vec!["--no-menu-bar", "--depth", "0"]] {
        let mut arguments = vec!["tree", fixture.target()];
        arguments.extend(suffix);
        let tree = run_ui(&arguments);
        assert_eq!(tree["root"]["role"], "AXApplication");
        assert_eq!(tree["root"]["depthLimited"], true);
        assert!(tree["root"].get("menuBarExcluded").is_none());
        assert!(tree["root"].get("childrenOmitted").is_none());
        assert!(tree["root"].get("children").is_none());
        assert_eq!(tree["narrowed"], true);
        assert_eq!(tree["truncated"], false);
    }
    for exclude in [false, true] {
        let mut arguments = vec!["tree", fixture.target(), "--depth", "1"];
        if exclude {
            arguments.push("--no-menu-bar");
        }
        let tree = run_ui(&arguments);
        assert_eq!(tree["narrowed"], true);
        assert_eq!(tree["truncated"], false);
        assert_eq!(tree["root"].get("menuBarExcluded").is_some(), exclude);
        assert!(children(&tree["root"]).iter().any(|node| node["role"] == "AXWindow"));
        for child in children(&tree["root"]) {
            assert!(child.get("children").is_none(), "{tree}");
            assert!(child.get("childrenOmitted").is_none(), "{tree}");
        }
        assert!(children(&tree["root"]).iter().any(|node| node["depthLimited"] == true));
    }
    let window = main_window_id(&fixture).to_string();
    let tree = run_ui(&["tree", fixture.target(), "--window", &window, "--depth", "0"]);
    assert_eq!(tree["root"]["role"], "AXWindow");
    assert_eq!(tree["root"]["depthLimited"], true);
    assert_eq!(tree["narrowed"], true);
    assert_eq!(tree["truncated"], false);
    assert!(tree["root"].get("children").is_none());
}
