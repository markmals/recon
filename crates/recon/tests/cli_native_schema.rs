//! The output schema every native command publishes, checked against the JSON those commands
//! actually print.
//!
//! The payloads here are the shapes the helper and the host really produce: the helper's own
//! per-field `status` objects, the window records that have no window server identity, the close
//! whose acknowledgment never arrived, and the correlation candidate that matched more than one
//! object. A schema that cannot accept one of them is wrong about the command it describes.

use std::process::Command;

use serde_json::{Value, json};

fn output(arguments: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_recon")).args(arguments).output().unwrap();
    assert!(output.status.success(), "{arguments:?}: {output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

/// The schema `recon schema <command>` publishes for that command's successful output.
fn schema(command: &[&str]) -> Value {
    let mut arguments = vec!["schema"];
    arguments.extend_from_slice(command);
    output(&arguments)["command"]["output"].clone()
}

fn validate(command: &[&str], instances: &[Value]) {
    let schema = schema(command);
    let validator = jsonschema::draft202012::new(&schema).unwrap();
    for instance in instances {
        let errors =
            validator.iter_errors(instance).map(|error| error.to_string()).collect::<Vec<_>>();
        assert!(errors.is_empty(), "{command:?}: {errors:?}; instance {instance}");
        let mut unexpected = instance.clone();
        unexpected.as_object_mut().unwrap().insert("unexpected".into(), json!(true));
        assert!(!validator.is_valid(&unexpected), "{command:?}: the root object must be closed");
    }
}

const SESSION: &str = "0123456789abcdef0123456789abcdef";
const HELPER_BUILD: &str = "e3b0c44298fc1c14";
const BUNDLE: &str = "/System/Applications/Calculator.app";

fn identity() -> Value {
    json!({
        "pid": 4242,
        "pidversion": 7,
        "uid": 501,
        "bundleId": "com.apple.calculator",
        "bundlePath": BUNDLE,
        "executablePath": "/System/Applications/Calculator.app/Contents/MacOS/Calculator",
        "architecture": "arm64e",
        "osBuild": "25A354",
        "osVersion": "Version 26.0 (Build 25A354)",
    })
}

/// A process that runs from a bare executable reports no bundle, and says so with nulls.
fn unbundled_identity() -> Value {
    json!({
        "pid": 4242,
        "pidversion": 7,
        "uid": 501,
        "bundleId": null,
        "bundlePath": null,
        "executablePath": null,
        "architecture": "arm64",
        "osBuild": "25A354",
        "osVersion": "Version 26.0 (Build 25A354)",
    })
}

fn observation(fields: Value, kind: &str, scope: Value) -> Value {
    json!({
        "observationId": "7d7f2b1c5e4a4f0b8c3d2e1f0a9b8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b",
        "source": "native-appkit",
        "sessionId": SESSION,
        "helperBuild": HELPER_BUILD,
        "protocol": 1,
        "identity": identity(),
        "scope": scope,
        "fields": fields,
        "kind": kind,
        "consistency": "main-thread-best-effort",
        "layerState": "model",
        "startTime": 1_761_600_000.123_4,
        "endTime": 1_761_600_000.456_7,
    })
}

fn default_scope() -> Value {
    json!({ "root": null, "windowId": null, "depth": 3, "limit": 200 })
}

fn coverage(complete: bool) -> Value {
    if complete {
        json!({ "visited": 37, "returned": 12, "narrowed": [], "truncated": [], "complete": true })
    } else {
        json!({
            "visited": 25_000,
            "returned": 200,
            "narrowed": ["depth", "selected-scope:absent-content-or-layer"],
            "truncated": ["returned:limit", "unsearched-scope", "work:25000"],
            "complete": false,
        })
    }
}

fn rect(space: &str, orientation: &str) -> Value {
    json!({
        "status": "value",
        "space": space,
        "units": "logical-points",
        "orientation": orientation,
        "x": 0.0,
        "y": 0.0,
        "width": 480.0,
        "height": 270.0,
    })
}

fn transform() -> Value {
    json!({
        "status": "value",
        "method": "primary-screen-height-flip",
        "orientation": "top-left",
        "primaryScreenHeight": 1169.0,
        "displayId": 1,
        "scale": 2.0,
    })
}

/// A window record exactly as the `windows` operation emits one, with no proven window server
/// identity: the native handle stands on its own and no id is borrowed for it.
fn native_only_window() -> Value {
    json!({
        "handle": "0000000000000001",
        "kind": "window",
        "relationships": {
            "parentWindow": { "status": "absent" },
            "contentView": "0000000000000002",
        },
        "basic": {
            "status": "value",
            "class": "NSWindow",
            "superclasses": ["NSResponder", "NSObject"],
            "title": { "status": "value", "value": "Calculator" },
            "identifier": { "status": "absent" },
            "frame": rect("global-screen", "bottom-left"),
            "normalizedFrame": {
                "screenTopLeft": rect("global-screen", "top-left"),
                "backingScaleFactor": { "status": "value", "value": 2.0 },
                "displayId": 1,
                "transform": transform(),
            },
            "accessibilityIdentifier": { "status": "value", "value": "main", "read": "target-executed" },
            "accessibilityRole": { "status": "absent", "read": "target-executed" },
            "visible": true,
            "key": false,
            "main": true,
            "windowNumber": { "status": "value", "value": 12_345 },
            "contentView": "0000000000000002",
            "childWindows": [],
            "sheet": { "status": "absent" },
            "topmostView": "0000000000000002",
        },
        "windowId": { "status": "unavailable" },
    })
}

/// The same record once the helper proved which window server window it is.
fn mapped_window() -> Value {
    let mut window = native_only_window();
    let object = window.as_object_mut().unwrap();
    object.insert("windowId".into(), json!(53_510));
    object.insert(
        "mapping".into(),
        json!({
            "method": "NSApplication.windowWithWindowNumber",
            "evidence": "WindowServer-owner-pid-and-NSApp-windows-membership",
        }),
    );
    window
}

/// A window the helper could not tell apart from another one keeps no id at all.
fn ambiguous_window() -> Value {
    let mut window = native_only_window();
    window.as_object_mut().unwrap().insert("mapping".into(), json!({ "status": "ambiguous" }));
    window
}

/// A view with every field group the helper can report, including the statuses that stand in for
/// values it could not read.
fn full_view() -> Value {
    json!({
        "handle": "0000000000000003",
        "kind": "view",
        "depth": 1,
        "relationships": {
            "superview": "0000000000000002",
            "window": "0000000000000001",
        },
        "basic": {
            "status": "value",
            "class": "NSButton",
            "superclasses": ["NSControl", "NSView", "NSResponder", "NSObject"],
            "identifier": { "status": "value", "value": "equals" },
            "frame": rect("superview", "top-left"),
            "bounds": rect("self", "bottom-left"),
            "flipped": false,
            "hidden": false,
            "alpha": { "status": "value", "value": 1.0 },
            "childCount": 0,
            "superview": "0000000000000002",
            "window": "0000000000000001",
            "normalizedFrame": {
                "window": rect("window", "bottom-left"),
                "screen": rect("global-screen", "bottom-left"),
                "screenTopLeft": rect("global-screen", "top-left"),
                "backingScaleFactor": { "status": "value", "value": 2.0 },
                "displayId": 1,
                "transform": transform(),
            },
            "accessibilityIdentifier": { "status": "unsupported" },
            "accessibilityRole": { "status": "value", "value": "AXButton", "read": "target-executed" },
        },
        "appearance": {
            "status": "value",
            "effectiveAppearance": "NSAppearanceNameAqua",
            "textColor": {
                "status": "value",
                "colorSpace": "sRGB",
                "catalog": "System",
                "name": "labelColor",
                "resolvedSpace": "deviceRGB",
                "components": {
                    "red": { "status": "value", "value": 0.0 },
                    "green": { "status": "value", "value": 0.0 },
                    "blue": { "status": "value", "value": 0.0 },
                    "alpha": { "status": "value", "value": 1.0 },
                },
            },
            "backgroundColor": { "status": "absent" },
        },
        "font": {
            "status": "value",
            "carrier": {
                "status": "value",
                "family": "SF Pro",
                "postScriptName": "SFPro-Regular",
                "pointSize": 13.0,
                "symbolicTraits": 0,
                "weight": { "status": "absent" },
                "descriptor": "SFPro-Regular",
            },
            "source": "view",
            "runs": [{
                "range": { "location": 0, "length": 5 },
                "font": {
                    "status": "value",
                    "family": "SF Pro",
                    "postScriptName": "SFPro-Semibold",
                    "pointSize": 13.0,
                    "symbolicTraits": 2,
                    "weight": 0.3,
                    "descriptor": "SFPro-Semibold",
                },
                "source": "attributed-run",
            }],
        },
        "layout": {
            "status": "value",
            "constraints": [{
                "owner": { "kind": "view", "handle": "0000000000000002" },
                "firstItem": { "kind": "view", "handle": "0000000000000003" },
                "secondItem": { "status": "absent" },
                "firstAttribute": 7,
                "secondAttribute": 0,
                "relation": 0,
                "multiplier": 1.0,
                "constant": 44.0,
                "priority": 1000.0,
                "active": true,
                "identifier": { "status": "absent" },
            }],
            "layoutGuides": [{ "kind": "layoutGuide", "handle": "0000000000000004" }],
            "intrinsicWidth": { "status": "value", "value": 44.0 },
            "intrinsicHeight": { "status": "no-intrinsic-metric" },
            "hugging": { "horizontal": 250.0, "vertical": 750.0 },
            "compressionResistance": { "horizontal": 750.0, "vertical": { "status": "non-finite" } },
        },
        "layer": {
            "status": "value",
            "layerState": "model",
            "handle": "0000000000000005",
            "frame": rect("parent-model-layer", "layer-geometry"),
            "bounds": rect("self-model-layer", "layer-geometry"),
            "transform": {
                "status": "value",
                "matrix": [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
            },
            "contentsScale": 2.0,
            "hidden": false,
            "opacity": 1.0,
            "cornerRadius": 6.0,
            "maskedCorners": 15,
            "borderWidth": 0.0,
            "shadowOpacity": 0.0,
            "shadowRadius": 3.0,
            "masksToBounds": false,
            "mask": { "status": "absent" },
            "backgroundColor": { "status": "absent" },
            "borderColor": { "status": "absent" },
            "shadowColor": {
                "status": "value",
                "colorSpace": "Generic RGB",
                "conversion": { "status": "unsupported" },
            },
        },
        "cell": {
            "status": "value",
            "handle": "0000000000000006",
            "class": "NSButtonCell",
            "cellType": 1,
            "state": -1,
            "enabled": true,
            "editable": false,
            "font": { "status": "absent" },
        },
    })
}

/// A view the walk could not read a group from, and one it only refers to because it already
/// returned it.
fn sparse_records() -> Vec<Value> {
    vec![
        json!({
            "handle": "0000000000000007",
            "kind": "layoutGuide",
            "depth": 2,
            "relationships": { "owningView": { "status": "absent" } },
            "basic": {
                "status": "value",
                "class": "NSLayoutGuide",
                "superclasses": ["NSObject"],
                "identifier": { "status": "value", "value": "NSView-safe-area" },
                "frame": { "status": "non-finite" },
                "owningView": { "status": "absent" },
            },
            "layout": { "status": "unsupported" },
            "font": { "status": "failed", "reason": "target-getter-exception" },
        }),
        json!({
            "handle": "0000000000000003",
            "kind": "view",
            "depth": 3,
            "reference": true,
        }),
        json!({
            "handle": "0000000000000008",
            "kind": "layer",
            "depth": 1,
            "relationships": { "status": "failed" },
            "basic": {
                "status": "value",
                "class": "CALayer",
                "superclasses": ["NSObject"],
                "frame": rect("parent-model-layer", "layer-geometry"),
                "bounds": rect("self-model-layer", "layer-geometry"),
                "hidden": false,
                "opacity": { "status": "value", "value": 1.0 },
                "childCount": 2,
                "superlayer": "0000000000000005",
                "layerState": "model",
            },
        }),
        json!({
            "handle": "0000000000000009",
            "kind": "view",
            "depth": 1,
            "relationships": { "superview": "0000000000000002", "window": "0000000000000001" },
            "basic": {
                "status": "value",
                "class": "_TtGC7SwiftUI14_NSHostingViewV8Fixture7ContentP_",
                "superclasses": ["NSView", "NSResponder", "NSObject"],
                "identifier": { "status": "absent" },
                "frame": rect("superview", "top-left"),
                "bounds": rect("self", "top-left"),
                "flipped": true,
                "hidden": false,
                "alpha": { "status": "non-finite" },
                "childCount": 1,
                "superview": "0000000000000002",
                "window": "0000000000000001",
                "normalizedFrame": { "status": "absent" },
                "accessibilityIdentifier": { "status": "absent", "read": "target-executed" },
                "accessibilityRole": { "status": "value", "value": "AXGroup", "read": "target-executed" },
                "hostingBoundary": {
                    "evidence": "runtime-class-name-heuristic",
                    "swiftUIContent": "not-inspected",
                },
            },
            "appearance": {
                "status": "value",
                "effectiveAppearance": { "status": "absent" },
                "visualEffect": { "material": 19, "blendingMode": 1, "state": 2, "emphasized": false },
            },
            "cell": { "status": "unsupported" },
        }),
    ]
}

fn nodes_response(kind: &str) -> Value {
    let mut nodes = vec![full_view()];
    nodes.extend(sparse_records());
    json!({
        "ok": true,
        "nodes": nodes,
        "windowRoots": [native_only_window()],
        "observation": observation(
            json!(["basic", "appearance", "font", "layout", "layer", "cell"]),
            kind,
            json!({ "root": null, "windowId": 53_510, "depth": 8, "limit": 500 }),
        ),
        "coverage": coverage(false),
    })
}

fn accessibility_tree() -> Value {
    json!({
        "application": { "path": BUNDLE, "pid": 4242 },
        "windowId": 53_510,
        "truncated": false,
        "narrowed": true,
        "root": {
            "role": "AXWindow",
            "title": "Calculator",
            "children": [{ "role": "AXButton", "title": "=" }],
        },
        "session": {
            "id": SESSION,
            "acquired": "attach",
            "profile": "standard",
            "protocolVersion": 1,
            "helperBuild": HELPER_BUILD,
        },
    })
}

#[test]
fn proposal0005_view_tree_and_find_accept_the_records_a_walk_really_returns() {
    for command in [["ui", "view", "tree"], ["ui", "view", "find"]] {
        validate(
            &command,
            &[
                nodes_response("view"),
                nodes_response("layer"),
                json!({
                    "ok": true,
                    "nodes": [],
                    "windowRoots": [],
                    "observation": observation(json!(["basic"]), "view", default_scope()),
                    "coverage": coverage(true),
                }),
            ],
        );
    }
}

#[test]
fn proposal0005_view_node_accepts_one_record_and_an_unbundled_target() {
    let mut unbundled = observation(json!(["basic"]), "view", default_scope());
    unbundled["identity"] = unbundled_identity();
    validate(
        &["ui", "view", "node"],
        &[
            json!({
                "ok": true,
                "node": full_view(),
                "observation": observation(
                    json!(["basic", "appearance", "font", "layout", "layer", "cell"]),
                    "view",
                    json!({ "root": "0000000000000003", "windowId": null, "depth": 3, "limit": 200 }),
                ),
                "coverage": coverage(true),
            }),
            json!({
                "ok": true,
                "node": native_only_window(),
                "observation": unbundled,
                "coverage": coverage(true),
            }),
        ],
    );
}

#[test]
fn proposal0005_view_fonts_accepts_grouped_descriptors_and_their_carriers() {
    validate(
        &["ui", "view", "fonts"],
        &[json!({
            "ok": true,
            "fonts": [
                {
                    "descriptor": {
                        "status": "value",
                        "family": "SF Pro",
                        "postScriptName": "SFPro-Regular",
                        "pointSize": 13.0,
                        "symbolicTraits": 0,
                        "weight": 0.0,
                        "descriptor": "SFPro-Regular",
                    },
                    "sources": [
                        { "handle": "0000000000000003", "kind": "view", "source": "view" },
                        {
                            "handle": "0000000000000006",
                            "kind": "cell",
                            "source": "attributed-run",
                            "range": { "location": 0, "length": 5 },
                        },
                    ],
                },
                {
                    "descriptor": {
                        "status": "value",
                        "family": "",
                        "postScriptName": "",
                        "pointSize": { "status": "non-finite" },
                        "symbolicTraits": 2,
                        "weight": { "status": "absent" },
                        "descriptor": "",
                    },
                    "sources": [{ "handle": "", "kind": "cell", "source": "cell" }],
                },
            ],
            "windowRoots": [mapped_window()],
            "observation": observation(json!(["font"]), "view", default_scope()),
            "coverage": coverage(false),
        })],
    );
}

#[test]
fn proposal0005_windows_keeps_its_unbound_shape_and_accepts_native_only_records() {
    let unbound = json!({
        "application": { "path": BUNDLE, "pid": 4242 },
        "windows": [{
            "id": 53_510,
            "title": null,
            "layer": 0,
            "bounds": { "x": 0.0, "y": 0.0, "width": 480.0, "height": 270.0 },
            "onScreen": true,
            "alpha": 1.0,
        }],
    });
    let mut bound = unbound.clone();
    let object = bound.as_object_mut().unwrap();
    object.insert(
        "session".into(),
        json!({
            "id": SESSION,
            "acquired": "existing",
            "profile": "research",
            "protocolVersion": 1,
            "helperBuild": HELPER_BUILD,
        }),
    );
    object.insert(
        "nativeWindows".into(),
        json!([native_only_window(), mapped_window(), ambiguous_window()]),
    );
    object.insert("coverage".into(), coverage(true));
    validate(&["ui", "windows"], &[unbound, bound]);
}

#[test]
fn proposal0005_correlate_accepts_both_full_observations_and_an_ambiguous_candidate() {
    validate(
        &["ui", "correlate"],
        &[json!({
            "native": nodes_response("view"),
            "accessibility": accessibility_tree(),
            "candidates": [
                {
                    "native": { "handle": "0000000000000003", "kind": "view", "class": "NSButton" },
                    "accessibility": {
                        "path": "AXWindow[0]/AXButton[2]",
                        "role": "AXButton",
                        "identifier": "equals",
                    },
                    "basis": "accessibility-identifier",
                    "ambiguous": false,
                },
                {
                    "native": { "handle": "0000000000000008", "kind": "layer", "class": null },
                    "accessibility": {
                        "path": "AXWindow[0]/AXGroup[1]",
                        "role": "AXGroup",
                        "identifier": null,
                    },
                    "basis": "role-and-bounds",
                    "ambiguous": true,
                },
            ],
            "unmatchedNative": [
                { "handle": "0000000000000007", "kind": "layoutGuide", "class": "NSLayoutGuide" },
            ],
            "unmatchedAccessibility": [
                { "path": "AXWindow[0]/AXStaticText[0]", "role": null, "identifier": null },
            ],
        })],
    );
}

#[test]
fn proposal0005_session_open_accepts_a_new_session_and_a_reused_one() {
    let opened = |reused: bool, identity: Value| {
        json!({
            "ok": true,
            "sessionId": SESSION,
            "reused": reused,
            "pid": 4242,
            "pidversion": 7,
            "uid": 501,
            "incarnation": { "pid": 4242, "pidversion": 7 },
            "identity": identity,
            "helperBuild": HELPER_BUILD,
            "protocol": 1,
            "profile": "standard",
            "mode": "existing",
            "requestedMode": "existing",
            "requestedProfile": "standard",
            "establishedMode": "attach",
            "capabilities": ["windows", "tree", "node", "find", "fonts"],
            "effects": ["helper-bootstrap-resident"],
            "helperResident": true,
            "bootstrapActive": true,
        })
    };
    validate(
        &["ui", "session", "open"],
        &[opened(false, identity()), opened(true, unbundled_identity())],
    );
}

#[test]
fn proposal0005_session_open_never_publishes_the_capability_it_withholds() {
    let validator = jsonschema::draft202012::new(&schema(&["ui", "session", "open"])).unwrap();
    let mut leaked = json!({
        "ok": true,
        "sessionId": SESSION,
        "reused": false,
        "pid": 4242,
        "pidversion": 7,
        "uid": 501,
        "incarnation": { "pid": 4242, "pidversion": 7 },
        "identity": identity(),
        "helperBuild": HELPER_BUILD,
        "protocol": 1,
        "profile": "standard",
        "mode": "attach",
        "requestedMode": "attach",
        "requestedProfile": "standard",
        "establishedMode": "attach",
        "capabilities": ["windows", "tree", "node", "find", "fonts"],
        "effects": ["helper-bootstrap-resident"],
        "helperResident": true,
        "bootstrapActive": true,
    });
    assert!(validator.is_valid(&leaked));
    leaked.as_object_mut().unwrap().insert("capability".into(), json!("0".repeat(64)));
    assert!(!validator.is_valid(&leaked), "a published capability must not validate");
}

#[test]
fn proposal0005_session_close_accepts_only_confirmed_states() {
    validate(
        &["ui", "session", "close"],
        &[
            json!({
                "sessionId": SESSION,
                "state": "closed",
                "acknowledged": true,
                "processExited": false,
                "helperResident": true,
                "bootstrapActive": true,
            }),
            json!({
                "sessionId": SESSION,
                "state": "already-closed",
                "acknowledged": true,
                "processExited": false,
                "helperResident": true,
                "bootstrapActive": true,
            }),
            json!({
                "sessionId": SESSION,
                "state": "closed-after-reconciliation",
                "acknowledged": true,
                "processExited": true,
                "helperResident": false,
                "bootstrapActive": false,
            }),
        ],
    );
}

fn fact(value: Value, source: &str) -> Value {
    json!({ "value": value, "source": source, "detail": null })
}

fn unknown_fact(source: &str, detail: &str) -> Value {
    json!({ "value": null, "source": source, "detail": detail })
}

fn host_facts() -> Value {
    json!({
        "osVersion": fact(json!("26.0"), "sw_vers"),
        "architecture": fact(json!("arm64"), "uname"),
        "sipDisabled": fact(json!(false), "csrutil status"),
        "amfiDisabled": fact(json!(false), "nvram boot-args"),
        "activeBootArgs": fact(json!([]), "sysctl kern.bootargs"),
        "configuredBootArgs": unknown_fact("nvram boot-args", "boot-args is not set"),
    })
}

/// The report `ui session preflight` prints for one bundle: modes with their own reasons, and
/// facts that are unknown rather than false when Recon could not read them.
fn preflight_report(pid: Value, running: Value) -> Value {
    json!({
            "application": { "path": BUNDLE, "pid": pid },
            "target": {
                "bundlePath": BUNDLE,
                "bundleId": "com.apple.calculator",
                "executablePath": fact(json!("/System/Applications/Calculator.app/Contents/MacOS/Calculator"), "bundle-executable"),
                "pid": pid,
                "runningPids": running,
                "processExecutable": unknown_fact("proc_pidpath", "no process is selected"),
                "architecture": unknown_fact("proc_pidinfo", "no process is selected"),
                "executableArchitectures": fact(json!(["arm64e"]), "mach-o header"),
                "signed": fact(json!(true), "csops"),
                "appleSigned": fact(json!(true), "codesign --verify"),
                "getTaskAllow": fact(json!(false), "codesign --display --entitlements"),
                "allowDyldEnvironmentVariables": fact(json!(false), "codesign --display --entitlements"),
                "hardenedRuntime": fact(json!(true), "codesign --display"),
                "libraryValidation": fact(json!(true), "codesign --display"),
                "platformBinary": fact(json!(true), "csops"),
            },
            "host": host_facts(),
            "helper": {
                "standard": {
                    "localAvailable": fact(json!(true), "standard-helper-artifacts"),
                    "localCompatible": unknown_fact("standard-build", "requires authenticated handshake"),
                    "resident": fact(json!(false), "selected-process"),
                    "residentCompatible": unknown_fact("standard-build", "requires authenticated handshake"),
                    "attachWithoutDialog": fact(json!(true), "standard-bound-loader"),
                    "inspectorThreadSetState": fact(json!(false), "standard-loader-entitlements"),
                },
                "research": {
                    "localAvailable": fact(json!(true), "research-helper-artifacts"),
                    "localCompatible": unknown_fact("research-build", "requires authenticated handshake"),
                    "resident": fact(json!(false), "selected-process"),
                    "residentCompatible": unknown_fact("research-build", "requires authenticated handshake"),
                    "attachWithoutDialog": fact(json!(true), "research-bound-loader"),
                    "inspectorThreadSetState": fact(json!(true), "research-loader-entitlements"),
                },
            },
            "modes": [
                {
                    "mode": "existing",
                    "profile": "standard",
                    "status": "blocked",
                    "reasons": ["no helper is resident in the selected process"],
                    "evidenceSources": ["authenticated-session-record+bootstrap-endpoint"],
                },
                {
                    "mode": "attach",
                    "profile": "research",
                    "status": "unknown",
                    "reasons": [],
                    "evidenceSources": [],
                },
            ],
    })
}

#[test]
fn proposal0005_preflight_accepts_a_report_about_a_bundle_with_no_selected_instance() {
    validate(
        &["ui", "session", "preflight"],
        &[
            preflight_report(json!(null), json!([])),
            preflight_report(json!(4242), json!([4242, 4243])),
        ],
    );
}

#[test]
fn proposal0005_prepare_host_accepts_both_a_status_report_and_an_applied_change() {
    validate(
        &["ui", "session", "prepare-host"],
        &[
            json!({
                "host": host_facts(),
                "requiredBootArgs": "amfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0",
                "readiness": "blocked",
                "operatorSteps": "To disable SIP: shut down the Mac; ...",
            }),
            json!({
                "action": "apply",
                "changed": true,
                "rebootRequired": true,
                "reconBootArgs": [
                    "amfi_get_out_of_my_way=0x1",
                    "-arm64e_preview_abi",
                    "tss_should_crash=0",
                ],
                "operatorSteps": ["To disable SIP: shut down the Mac; ..."],
            }),
            json!({
                "action": "revert",
                "changed": true,
                "rebootRequired": true,
                "reconBootArgs": [],
                "operatorSteps": ["To re-enable SIP, return to Recovery, run `csrutil enable`."],
            }),
        ],
    );
}

/// A schema that accepts everything describes nothing. These are shapes the helper and the host
/// never print, and each one names a promise the schema is keeping.
#[test]
fn proposal0005_published_schemas_reject_shapes_the_helper_never_prints() {
    let tree = jsonschema::draft202012::new(&schema(&["ui", "view", "tree"])).unwrap();

    let mut invented_status = nodes_response("view");
    invented_status["nodes"][0]["basic"]["identifier"] = json!({ "status": "probably" });
    assert!(!tree.is_valid(&invented_status), "the status vocabulary is closed");

    let mut invented_member = nodes_response("view");
    invented_member["nodes"][0]["sublayers"] = json!([]);
    assert!(!tree.is_valid(&invented_member), "a record reports no member it did not observe");

    let mut borrowed_handle = nodes_response("view");
    borrowed_handle["nodes"][0]["relationships"]["window"] =
        json!({ "handle": "0000000000000001" });
    assert!(!tree.is_valid(&borrowed_handle), "a relationship is a handle or an explicit absence");

    let mut partial_coverage = nodes_response("view");
    partial_coverage["coverage"].as_object_mut().unwrap().remove("complete");
    assert!(!tree.is_valid(&partial_coverage), "coverage always answers whether it is complete");

    let mut relabelled_kind = nodes_response("view");
    relabelled_kind["nodes"][0]["kind"] = json!("layout-guide");
    assert!(!tree.is_valid(&relabelled_kind), "a kind keeps the helper's own spelling");

    let close = jsonschema::draft202012::new(&schema(&["ui", "session", "close"])).unwrap();
    assert!(
        !close.is_valid(&json!({
            "sessionId": SESSION,
            "state": "closed",
            "acknowledged": true,
            "helperResident": null,
            "bootstrapActive": null,
        })),
        "a settled close reports residency rather than leaving it unknown"
    );
    assert!(
        !close.is_valid(&json!({
            "sessionId": SESSION,
            "state": "close-unconfirmed",
            "acknowledged": false,
            "processExited": false,
            "helperResident": true,
            "bootstrapActive": true,
        })),
        "an unconfirmed close never claims a confirmed residency"
    );

    let preflight = jsonschema::draft202012::new(&schema(&["ui", "session", "preflight"])).unwrap();
    let mut textual_pid = preflight_report(json!(4242), json!([4242]));
    textual_pid["application"]["pid"] = json!("4242");
    assert!(!preflight.is_valid(&textual_pid), "a pid is a number or nothing");
}

/// The facts this host really reports, against the schema the registry really publishes. These
/// two commands consult the host and change nothing, so they can be run exactly as a caller
/// would.
#[cfg(target_os = "macos")]
#[test]
fn proposal0005_real_preflight_and_host_status_validate_against_their_schemas() {
    validate(&["ui", "session", "preflight"], &[output(&["ui", "session", "preflight", BUNDLE])]);
    validate(
        &["ui", "session", "prepare-host"],
        &[output(&["ui", "session", "prepare-host", "--status"])],
    );
}
