use std::path::Path;
use std::process::Command;

use recon::application::Pid;
use recon::ui::response::{ObservedApplication, TreeResponse};
use recon::ui::tree::{AttributeValue, ElementNode, Node, ReadFailure};
use serde_json::{Value, json};

fn output(arguments: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_recon")).args(arguments).output().unwrap();
    assert!(output.status.success(), "{arguments:?}: {:?}", output);
    serde_json::from_slice(&output.stdout).unwrap()
}

fn validate(command: &[&str], instance: &Value) {
    let mut arguments = vec!["schema"];
    arguments.extend_from_slice(command);
    let specification = output(&arguments);
    let schema = &specification["command"]["output"];
    let validator = jsonschema::draft202012::new(schema).unwrap();
    let errors = validator.iter_errors(instance).map(|error| error.to_string()).collect::<Vec<_>>();
    assert!(errors.is_empty(), "{command:?}: {errors:?}; instance {instance}");
    let mut unexpected = instance.clone();
    unexpected.as_object_mut().unwrap().insert("unexpected".into(), json!(true));
    assert!(!validator.is_valid(&unexpected), "root object must be closed");
}

#[test]
fn proposal0003_real_meta_outputs_validate_against_advertised_schemas() {
    validate(&["schema"], &output(&["schema"]));
    validate(&["schema"], &output(&["schema", "context"]));
    validate(&["context"], &output(&["context"]));
}

#[test]
fn proposal0003_tree_schema_tracks_handwritten_serialization_and_omission() {
    for (truncated, children) in [
        (false, Vec::new()),
        (
            true,
            vec![
                Node::Repeated { role: Some("AXButton".into()) },
                Node::Failed(ReadFailure::Unavailable),
                Node::Failed(ReadFailure::Timeout),
                Node::Failed(ReadFailure::Failed(-25202)),
            ],
        ),
    ] {
        let root = Node::Element(Box::new(ElementNode {
            role: Some("AXWindow".into()),
            subrole: None,
            title: None,
            description: None,
            value: Some(AttributeValue::Text("bounded".into())),
            value_truncated: truncated,
            identifier: None,
            enabled: None,
            focused: None,
            frame: None,
            actions: Vec::new(),
            children,
            children_omitted: truncated,
            depth_limited: false,
            menu_bar_excluded: false,
        }));
        let response = TreeResponse {
            application: ObservedApplication { path: Path::new("/Example.app"), pid: Pid(42) },
            window_id: None,
            truncated,
            narrowed: false,
            root,
        };
        validate(&["ui", "tree"], &serde_json::to_value(response).unwrap());
    }
}

#[test]
fn proposal0003_real_usage_errors_validate_and_are_declared() {
    let index = output(&["schema"]);
    let validator = jsonschema::draft202012::new(&index["errors"]["schema"]).unwrap();
    for arguments in [
        vec!["ui", "tree", "Notes", "--window", "abc"],
        vec!["app", "list", "--json"],
        vec!["schema", "bogus"],
        vec!["context", "extra"],
    ] {
        let response = Command::new(env!("CARGO_BIN_EXE_recon")).args(&arguments).output().unwrap();
        let envelope: Value = serde_json::from_slice(&response.stderr).unwrap();
        assert!(validator.is_valid(&envelope), "{envelope}");
        let command = envelope["error"]["command"].as_array().unwrap();
        let mut path = vec!["schema"];
        path.extend(command.iter().map(|word| word.as_str().unwrap()));
        let specification = output(&path);
        assert!(
            specification["command"]["errors"]
                .as_array()
                .unwrap()
                .contains(&envelope["error"]["code"])
        );
    }
}

#[cfg(target_os = "macos")]
#[test]
fn proposal0003_inspection_output_and_runtime_failure_validate() {
    validate(&["app", "list"], &output(&["app", "list"]));
    validate(
        &["app", "inspect"],
        &output(&["app", "inspect", "/System/Applications/Calculator.app"]),
    );
    let response = Command::new(env!("CARGO_BIN_EXE_recon"))
        .args(["app", "inspect", "/recon-no-such.app"])
        .output()
        .unwrap();
    assert_eq!(response.status.code(), Some(1));
    assert!(response.stdout.is_empty());
    let envelope: Value = serde_json::from_slice(&response.stderr).unwrap();
    assert_eq!(envelope["error"]["code"], "NOT_FOUND");
    let index = output(&["schema"]);
    assert!(jsonschema::draft202012::is_valid(&index["errors"]["schema"], &envelope));
}

#[cfg(target_os = "macos")]
#[test]
fn proposal0003_actual_launch_output_validates() {
    use std::io::Write;
    use std::process::Stdio;
    let scratch = tempfile::tempdir().unwrap();
    let bundle = scratch.path().join("Schema Fixture.app");
    let executable = bundle.join("Contents/MacOS/fixture");
    std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
    std::fs::write(bundle.join("Contents/Info.plist"), r#"<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>test.recon.schema-fixture</string><key>CFBundleExecutable</key><string>fixture</string><key>LSUIElement</key><true/></dict></plist>"#).unwrap();
    let mut compiler = Command::new("cc")
        .args(["-x", "objective-c", "-framework", "AppKit", "-o"])
        .arg(&executable)
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    compiler.stdin.take().unwrap().write_all(b"#import <AppKit/AppKit.h>\nint main(void) { @autoreleasepool { [NSApplication sharedApplication]; [NSApp run]; } }\n").unwrap();
    assert!(compiler.wait().unwrap().success());
    let launched = output(&["app", "launch", bundle.to_str().unwrap()]);
    struct RunningFixture(u32);
    impl Drop for RunningFixture {
        fn drop(&mut self) {
            let _ = Command::new("/bin/kill").arg(self.0.to_string()).status();
        }
    }
    let _running = RunningFixture(launched["pid"].as_u64().unwrap().try_into().unwrap());
    validate(&["app", "launch"], &launched);
}
