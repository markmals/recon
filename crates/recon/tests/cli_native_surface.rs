//! Proposal 0005 §1 and §6: the native inspection command surface.
//!
//! These tests cover the grammar, the published specification, and the error contract only.
//! Nothing here opens a session, loads code, or reads a native object: a command that needs a
//! target-side helper is exercised up to the point where the request is parsed and described.

use std::process::{Command, Output};

use recon::application::Pid;
use recon::error::{
    ErrorCode, Failure, InspectionFailure, InspectionStage, Interference, InterferenceEvidence,
};
use recon::inspection::SessionId;
use recon::request::{Request, parse_arguments};
use recon::ui::request::{
    AccessMode, AccessProfile, FieldGroup, FieldSet, Filters, HostPreparation, NativeScope,
    NodeHandle, Selection, SessionRequest, TraversalKind, UiRequest, ViewRequest,
};
use recon::ui::window::WindowId;
use serde_json::{Value, json};
mod support;

/// A syntactically valid session identifier: 32 lowercase hex digits, as `ui session open` mints.
const SESSION: &str = "0123456789abcdef0123456789abcdef";
/// A syntactically valid native handle: 16 lowercase hex digits.
const HANDLE: &str = "00112233aabbccdd";

const NATIVE_CODES: [&str; 8] = [
    "INSPECTION_UNAVAILABLE",
    "INSPECTION_PROTOCOL_MISMATCH",
    "INSPECTION_FAILED",
    "INSPECTION_TIMEOUT",
    "INSPECTION_BUSY",
    "STALE_SESSION",
    "STALE_NODE",
    "UI_INTERFERENCE",
];

const NEW_COMMANDS: [&[&str]; 9] = [
    &["ui", "session", "preflight"],
    &["ui", "session", "prepare-host"],
    &["ui", "session", "open"],
    &["ui", "session", "close"],
    &["ui", "view", "tree"],
    &["ui", "view", "node"],
    &["ui", "view", "find"],
    &["ui", "view", "fonts"],
    &["ui", "correlate"],
];

fn recon(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_recon")).args(arguments).output().unwrap()
}

fn success(arguments: &[&str]) -> Vec<u8> {
    let output = recon(arguments);
    assert_eq!(output.status.code(), Some(0), "{arguments:?}: {output:?}");
    assert!(output.stderr.is_empty(), "{arguments:?}: {output:?}");
    output.stdout
}

fn json_output(arguments: &[&str]) -> Value {
    serde_json::from_slice(&success(arguments)).unwrap()
}

fn specification(path: &[&str]) -> Value {
    let mut arguments = vec!["schema"];
    arguments.extend_from_slice(path);
    json_output(&arguments)
}

fn usage_error(arguments: &[&str]) -> Value {
    let output = recon(arguments);
    assert_eq!(output.status.code(), Some(2), "{arguments:?}: {output:?}");
    assert!(output.stdout.is_empty(), "{arguments:?}: {output:?}");
    let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
    support::validate_failure(&envelope, &[]);
    assert_eq!(envelope["error"]["code"], "INVALID_ARGUMENT", "{envelope}");
    envelope["error"].clone()
}

fn option_named<'a>(specification: &'a Value, name: &str) -> &'a Value {
    specification["command"]["options"]
        .as_array()
        .unwrap_or_else(|| panic!("options array: {specification}"))
        .iter()
        .find(|option| option["name"] == name)
        .unwrap_or_else(|| panic!("no {name}: {specification}"))
}

fn option_names(specification: &Value) -> Vec<String> {
    specification["command"]["options"]
        .as_array()
        .unwrap()
        .iter()
        .map(|option| option["name"].as_str().unwrap().to_owned())
        .collect()
}

fn parse(words: &[&str]) -> Result<UiRequest, Failure> {
    let arguments: Vec<String> = words.iter().map(|word| (*word).to_owned()).collect();
    parse_arguments(&arguments).map(|request| match request {
        Request::Ui(request) => request,
        other => panic!("{words:?}: expected a ui request, got {other:?}"),
    })
}

fn rejects(words: &[&str]) -> Failure {
    match parse(words) {
        Ok(request) => panic!("{words:?} should not parse, got {request:?}"),
        Err(failure) => {
            support::validate_failure(&failure.report(), &[]);
            assert_eq!(failure.code, ErrorCode::InvalidArgument, "{words:?}");
            failure
        }
    }
}

fn session() -> SessionId {
    SessionId::parse(SESSION).unwrap()
}

fn handle() -> NodeHandle {
    NodeHandle::parse(HANDLE).unwrap()
}

fn selection(target: &str) -> Selection {
    Selection { target: target.to_owned(), pid: None }
}

// ---------------------------------------------------------------- discovery

#[test]
fn the_registry_exposes_every_nested_native_leaf() {
    let index = json_output(&["schema"]);
    let paths: Vec<Value> =
        index["commands"].as_array().unwrap().iter().map(|entry| entry["path"].clone()).collect();
    for command in NEW_COMMANDS {
        assert!(paths.contains(&json!(command)), "missing {command:?}: {paths:?}");
    }
    assert!(!paths.contains(&json!(["ui", "tree"])), "{paths:?}");
    assert!(!paths.contains(&json!(["ui", "view"])), "a group is not a leaf: {paths:?}");
    assert!(!paths.contains(&json!(["ui", "session"])), "a group is not a leaf: {paths:?}");
}

#[test]
fn each_group_lists_exactly_its_own_operations() {
    assert_eq!(
        usage_error(&["ui", "session"])["details"]["accepted"],
        json!(["preflight", "prepare-host", "open", "close"])
    );
    assert_eq!(
        usage_error(&["ui", "view"])["details"]["accepted"],
        json!(["tree", "node", "find", "fonts"])
    );
    let accepted = usage_error(&["ui"])["details"]["accepted"].clone();
    assert_eq!(
        accepted,
        json!(["windows", "ax", "screenshot", "session", "view", "correlate"]),
        "the ui group lists its subgroups beside its own operations"
    );
    for group in [vec!["ui", "session"], vec!["ui", "view"]] {
        let mut help = group.clone();
        help.push("--help");
        let rendered = String::from_utf8(success(&help)).unwrap();
        assert!(rendered.contains(group[1]), "{rendered}");
    }
}

#[test]
fn every_native_leaf_publishes_its_fully_qualified_specification() {
    let index = json_output(&["schema"]);
    let codes = index["errors"]["codes"].as_array().unwrap();
    for command in NEW_COMMANDS {
        let specification = specification(command);
        assert_eq!(specification["command"]["path"], json!(command), "{command:?}");
        assert_eq!(
            specification["command"]["output"]["$schema"],
            "https://json-schema.org/draft/2020-12/schema",
            "{command:?}"
        );
        for code in specification["command"]["errors"].as_array().unwrap() {
            assert!(codes.iter().any(|known| known["code"] == *code), "{command:?}: {code}");
        }
        let mut help = command.to_vec();
        help.push("--help");
        success(&help);
    }
}

#[test]
fn the_retired_and_never_shipped_spellings_stay_rejected() {
    assert_eq!(usage_error(&["ui", "tree", "Calculator"])["details"]["value"], "tree");
    assert_eq!(usage_error(&["ax", "tree", "Calculator"])["details"]["value"], "ax");
    assert_eq!(usage_error(&["native", "tree", "Calculator"])["details"]["value"], "native");
    assert_eq!(usage_error(&["view", "tree", "Calculator"])["details"]["value"], "view");
    assert_eq!(usage_error(&["session", "open", "Calculator"])["details"]["value"], "session");
    let accepted = usage_error(&["ui", "view", "treee"])["details"]["accepted"].clone();
    assert_eq!(accepted, json!(["tree", "node", "find", "fonts"]));
}

// ------------------------------------------------------- published options

#[test]
fn traversal_options_publish_their_defaults_and_ranges() {
    let tree = specification(&["ui", "view", "tree"]);
    assert_eq!(
        option_names(&tree),
        ["--session", "--pid", "--window", "--root", "--kind", "--depth", "--fields", "--limit"]
    );

    let depth = option_named(&tree, "--depth");
    assert_eq!(depth["default"], json!(3));
    assert_eq!(depth["constraints"]["minimum"], json!(0));
    assert_eq!(depth["constraints"]["maximum"], json!(64));
    assert_eq!(depth["type"], "integer");

    let limit = option_named(&tree, "--limit");
    assert_eq!(limit["default"], json!(200));
    assert_eq!(limit["constraints"]["minimum"], json!(1));
    assert_eq!(limit["constraints"]["maximum"], json!(25000));

    let kind = option_named(&tree, "--kind");
    assert_eq!(kind["default"], json!("view"));
    assert_eq!(kind["constraints"]["enum"], json!(["view", "layer"]));

    let session = option_named(&tree, "--session");
    assert_eq!(session["required"], json!(true));
    assert_eq!(session["constraints"]["pattern"], json!("^[0-9a-f]{32}$"));

    let root = option_named(&tree, "--root");
    assert_eq!(root["constraints"]["pattern"], json!("^[0-9a-f]{16}$"));
}

#[test]
fn the_field_groups_are_a_published_non_empty_set() {
    let tree = specification(&["ui", "view", "tree"]);
    let fields = option_named(&tree, "--fields");
    assert_eq!(fields["default"], json!("basic"));
    assert_eq!(
        fields["constraints"]["accepted"],
        json!(["basic", "appearance", "font", "layout", "layer", "cell"])
    );
    assert_eq!(fields["constraints"]["separator"], json!(","));
    assert_eq!(fields["constraints"]["unique"], json!(true));
    let validator = jsonschema::draft202012::new(&fields["constraints"]).unwrap();
    for (value, accepted) in
        [("basic", true), ("layer,cell", true), ("", false), ("basic,bogus", false), (",", false)]
    {
        assert_eq!(validator.is_valid(&json!(value)), accepted, "{value:?}");
    }
}

#[test]
fn session_open_publishes_a_required_mode_a_default_profile_and_its_usage_rules() {
    let open = specification(&["ui", "session", "open"]);
    let mode = option_named(&open, "--mode");
    assert_eq!(mode["required"], json!(true));
    assert_eq!(mode["default"], Value::Null, "--mode has no default");
    assert_eq!(mode["constraints"]["enum"], json!(["existing", "launch", "attach"]));

    let profile = option_named(&open, "--profile");
    assert_eq!(profile["required"], json!(false));
    assert_eq!(profile["default"], json!("standard"));
    assert_eq!(profile["constraints"]["enum"], json!(["standard", "research"]));

    assert_eq!(option_named(&open, "--allow-authorization")["type"], "boolean");
    let usage = open["command"]["usage"].as_array().unwrap();
    assert!(
        usage.contains(&json!({
            "subject": "--allow-authorization",
            "relation": "requires",
            "objects": ["--mode attach"],
        })),
        "{usage:?}"
    );
    assert!(
        usage.contains(&json!({
            "subject": "--pid",
            "relation": "forbids",
            "objects": ["--mode launch"],
        })),
        "{usage:?}"
    );

    let find = specification(&["ui", "view", "find"]);
    assert!(
        find["command"]["usage"].as_array().unwrap().contains(&json!({
            "subject": "ui view find",
            "relation": "one-of",
            "objects": ["--class", "--identifier"],
        })),
        "{find}"
    );
    for command in [vec!["ui", "view", "tree"], vec!["ui", "view", "find"]] {
        let published = specification(&command);
        assert!(
            published["command"]["usage"].as_array().unwrap().contains(&json!({
                "subject": "--kind layer",
                "relation": "one-of",
                "objects": ["--window", "--root"],
            })),
            "{published}"
        );
    }
}

#[test]
fn the_optional_session_joins_the_existing_external_commands() {
    for command in [
        vec!["ui", "windows"],
        vec!["ui", "ax", "tree"],
        vec!["ui", "screenshot"],
        vec!["ui", "correlate"],
    ] {
        let published = specification(&command);
        let session = option_named(&published, "--session");
        let required = command == ["ui", "correlate"];
        assert_eq!(session["required"], json!(required), "{command:?}");
        assert_eq!(session["constraints"]["pattern"], json!("^[0-9a-f]{32}$"), "{command:?}");
    }
    assert_eq!(option_named(&specification(&["ui", "correlate"]), "--window")["required"], true);
}

#[test]
fn effects_and_permissions_distinguish_loading_launching_and_reading() {
    let open = specification(&["ui", "session", "open"]);
    let effects = open["command"]["effects"].as_array().unwrap();
    for effect in ["loads-code-into-target", "launches-application", "executes-target-code"] {
        assert!(effects.contains(&json!(effect)), "{effects:?}");
    }
    assert!(!effects.contains(&json!("activates-application")), "{effects:?}");

    let prepare = specification(&["ui", "session", "prepare-host"]);
    let effects = prepare["command"]["effects"].as_array().unwrap();
    assert!(effects.contains(&json!("changes-host-security")), "{effects:?}");
    assert!(effects.contains(&json!("requires-administrator")), "{effects:?}");

    let preflight = specification(&["ui", "session", "preflight"]);
    assert_eq!(preflight["command"]["effects"], json!([]));
    assert_eq!(preflight["command"]["permissions"], json!([]));

    let tree = specification(&["ui", "view", "tree"]);
    assert_eq!(tree["command"]["effects"], json!(["executes-target-code"]));

    let correlate = specification(&["ui", "correlate"]);
    let permissions: Vec<&str> = correlate["command"]["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|permission| permission["permission"].as_str().unwrap())
        .collect();
    assert!(permissions.contains(&"accessibility"), "{permissions:?}");
}

// --------------------------------------------------------- error contract

#[test]
fn the_error_index_publishes_the_native_classifications() {
    let index = json_output(&["schema"]);
    let codes = index["errors"]["codes"].as_array().unwrap();
    for code in NATIVE_CODES {
        let published = codes
            .iter()
            .find(|known| known["code"] == code)
            .unwrap_or_else(|| panic!("{code} is not published"));
        assert_eq!(published["exitCode"], json!(1), "{code}");
        assert_eq!(published["retryable"], json!(false), "{code}");
        assert!(published["summary"].as_str().unwrap().len() > 10, "{code}");
    }
}

#[test]
fn native_commands_declare_the_failures_they_can_report() {
    let tree = specification(&["ui", "view", "tree"]);
    let declared: Vec<&str> = tree["command"]["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|code| code.as_str().unwrap())
        .collect();
    for code in NATIVE_CODES {
        assert!(declared.contains(&code), "ui view tree: {code} missing from {declared:?}");
    }

    let windows = specification(&["ui", "windows"]);
    let declared: Vec<&str> = windows["command"]["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|code| code.as_str().unwrap())
        .collect();
    for code in ["STALE_SESSION", "INSPECTION_UNAVAILABLE", "UI_INTERFERENCE"] {
        assert!(declared.contains(&code), "ui windows: {code} missing from {declared:?}");
    }
    assert!(!declared.contains(&"STALE_NODE"), "{declared:?}");
}

#[test]
fn native_failure_details_validate_against_the_published_error_schema() {
    let index = json_output(&["schema"]);
    let validator = jsonschema::draft202012::new(&index["errors"]["schema"]).unwrap();

    let timeout = Failure::new(
        ErrorCode::InspectionTimeout,
        "the native read did not finish within two seconds",
        InspectionFailure::new(InspectionStage::Read)
            .session(SESSION)
            .pid(412)
            .node(HANDLE)
            .detail("the main thread did not answer")
            .target_execution_may_continue()
            .details(),
    )
    .with_command(&["ui", "view", "tree"]);
    let envelope = serde_json::to_value(timeout.report()).unwrap();
    assert!(validator.is_valid(&envelope), "{envelope}");
    assert_eq!(envelope["error"]["details"]["targetExecutionMayContinue"], json!(true));
    assert_eq!(envelope["error"]["details"]["stage"], json!("read"));
    assert_eq!(envelope["error"]["retryable"], json!(false));

    let busy = Failure::new(
        ErrorCode::InspectionBusy,
        "another native read is outstanding for this process",
        InspectionFailure::new(InspectionStage::Open)
            .session(SESSION)
            .recovery("recon ui session close SESSION")
            .details(),
    );
    assert!(validator.is_valid(&serde_json::to_value(busy.report()).unwrap()));

    let interference = Failure::new(
        ErrorCode::UiInterference,
        "the target activated a window during the read",
        Interference::new(InspectionStage::Read, InterferenceEvidence::ForegroundApplication)
            .pid(412)
            .window(53510)
            .disturbance("the foreground application changed")
            .details(),
    );
    let envelope = serde_json::to_value(interference.report()).unwrap();
    assert!(validator.is_valid(&envelope), "{envelope}");
    assert_eq!(envelope["error"]["details"]["evidenceSource"], json!("foreground-application"));

    let fabricated = json!({
        "error": {
            "code": "INSPECTION_FAILED",
            "message": "invented details",
            "retryable": false,
            "command": null,
            "details": {"stage": "teleport"},
        }
    });
    assert!(!validator.is_valid(&fabricated), "an unknown stage is not a published detail shape");
}

// ------------------------------------------------------------- the grammar

#[test]
fn session_open_parses_its_mode_profile_and_authorization() {
    assert_eq!(
        parse(&["ui", "session", "open", "Notes", "--mode", "attach"]).unwrap(),
        UiRequest::Session(SessionRequest::Open {
            selection: selection("Notes"),
            mode: AccessMode::Attach,
            profile: AccessProfile::Standard,
            allow_authorization: false,
        })
    );
    assert_eq!(
        parse(&[
            "ui",
            "session",
            "open",
            "Notes",
            "--mode",
            "attach",
            "--profile",
            "research",
            "--pid",
            "412",
            "--allow-authorization",
        ])
        .unwrap(),
        UiRequest::Session(SessionRequest::Open {
            selection: Selection { target: "Notes".to_owned(), pid: Some(Pid(412)) },
            mode: AccessMode::Attach,
            profile: AccessProfile::Research,
            allow_authorization: true,
        })
    );
    rejects(&["ui", "session", "open", "Notes"]);
    rejects(&["ui", "session", "open", "Notes", "--mode"]);
    rejects(&["ui", "session", "open", "Notes", "--mode", "ATTACH"]);
    rejects(&["ui", "session", "open", "Notes", "--mode", "replace"]);
    rejects(&["ui", "session", "open", "Notes", "--mode", "attach", "--profile", "lab"]);
}

#[test]
fn authorization_belongs_only_to_attach_and_launch_refuses_an_instance() {
    for mode in ["existing", "launch"] {
        let failure =
            rejects(&["ui", "session", "open", "Notes", "--mode", mode, "--allow-authorization"]);
        let details = failure.details.unwrap();
        assert_eq!(details["argument"], "--allow-authorization");
        assert_eq!(details["value"], mode);
        assert_eq!(details["expected"], "--mode attach");
        assert_eq!(failure.command.unwrap(), ["ui", "session", "open"]);
    }
    let failure = rejects(&["ui", "session", "open", "Notes", "--mode", "launch", "--pid", "412"]);
    let details = failure.details.unwrap();
    assert_eq!(details["argument"], "--pid");
    assert_eq!(details["value"], "launch");
    assert!(
        parse(&["ui", "session", "open", "Notes", "--mode", "existing", "--pid", "412"]).is_ok()
    );
}

#[test]
fn session_lifecycle_commands_take_their_own_arguments() {
    assert_eq!(
        parse(&["ui", "session", "preflight", "Notes", "--pid", "9"]).unwrap(),
        UiRequest::Session(SessionRequest::Preflight {
            selection: Selection { target: "Notes".to_owned(), pid: Some(Pid(9)) },
        })
    );
    assert_eq!(
        parse(&["ui", "session", "prepare-host"]).unwrap(),
        UiRequest::Session(SessionRequest::PrepareHost { action: HostPreparation::Apply })
    );
    assert_eq!(
        parse(&["ui", "session", "prepare-host", "--status"]).unwrap(),
        UiRequest::Session(SessionRequest::PrepareHost { action: HostPreparation::Status })
    );
    assert_eq!(
        parse(&["ui", "session", "prepare-host", "--revert"]).unwrap(),
        UiRequest::Session(SessionRequest::PrepareHost { action: HostPreparation::Revert })
    );
    rejects(&["ui", "session", "prepare-host", "--status", "--revert"]);
    rejects(&["ui", "session", "prepare-host", "Notes"]);

    assert_eq!(
        parse(&["ui", "session", "close", SESSION]).unwrap(),
        UiRequest::Session(SessionRequest::Close { session: session() })
    );
    rejects(&["ui", "session", "close"]);
    rejects(&["ui", "session", "close", "Notes"]);
    rejects(&["ui", "session", "close", &SESSION.to_uppercase()]);
    rejects(&["ui", "session", "close", &SESSION[..31]]);
}

#[test]
fn view_commands_require_a_session_and_default_their_bounds() {
    assert_eq!(
        parse(&["ui", "view", "tree", "Notes", "--session", SESSION]).unwrap(),
        UiRequest::View(ViewRequest::Tree {
            selection: selection("Notes"),
            session: session(),
            scope: NativeScope { window: None, root: None },
            kind: TraversalKind::View,
            depth: 3,
            fields: FieldSet::default(),
            limit: 200,
        })
    );
    assert_eq!(
        parse(&[
            "ui",
            "view",
            "tree",
            "Notes",
            "--session",
            SESSION,
            "--window",
            "53510",
            "--root",
            HANDLE,
            "--kind",
            "layer",
            "--depth",
            "0",
            "--fields",
            "layer,font",
            "--limit",
            "25000",
        ])
        .unwrap(),
        UiRequest::View(ViewRequest::Tree {
            selection: selection("Notes"),
            session: session(),
            scope: NativeScope { window: Some(WindowId(53510)), root: Some(handle()) },
            kind: TraversalKind::Layer,
            depth: 0,
            fields: FieldSet::parse("layer,font").unwrap(),
            limit: 25000,
        })
    );
    assert_eq!(
        parse(&["ui", "view", "node", "Notes", HANDLE, "--session", SESSION]).unwrap(),
        UiRequest::View(ViewRequest::Node {
            selection: selection("Notes"),
            session: session(),
            handle: handle(),
            fields: FieldSet::default(),
        })
    );
    assert_eq!(
        parse(&["ui", "view", "fonts", "Notes", "--session", SESSION, "--window", "7"]).unwrap(),
        UiRequest::View(ViewRequest::Fonts {
            selection: selection("Notes"),
            session: session(),
            scope: NativeScope { window: Some(WindowId(7)), root: None },
            depth: 3,
            limit: 200,
        })
    );
    for words in [
        vec!["ui", "view", "tree", "Notes"],
        vec!["ui", "view", "node", "Notes", HANDLE],
        vec!["ui", "view", "find", "Notes", "--class", "NSButton"],
        vec!["ui", "view", "fonts", "Notes"],
        vec!["ui", "correlate", "Notes", "--window", "7"],
    ] {
        rejects(&words);
    }
    rejects(&["ui", "view", "node", "Notes", "--session", SESSION]);
    rejects(&["ui", "view", "node", "Notes", "zz112233aabbccdd", "--session", SESSION]);
    rejects(&["ui", "view", "tree", "Notes", "--session", SESSION, "--depth", "65"]);
    rejects(&["ui", "view", "tree", "Notes", "--session", SESSION, "--limit", "0"]);
    rejects(&["ui", "view", "tree", "Notes", "--session", SESSION, "--limit", "25001"]);
    rejects(&["ui", "view", "tree", "Notes", "--session", SESSION, "--kind", "cell"]);
    rejects(&["ui", "view", "fonts", "Notes", "--session", SESSION, "--kind", "layer"]);
    rejects(&["ui", "view", "fonts", "Notes", "--session", SESSION, "--fields", "basic"]);
}

#[test]
fn a_field_set_is_a_non_empty_set_of_known_groups() {
    assert_eq!(FieldSet::default().groups(), [FieldGroup::Basic]);
    assert_eq!(
        FieldSet::parse("basic,appearance,font,layout,layer,cell").unwrap().groups(),
        [
            FieldGroup::Basic,
            FieldGroup::Appearance,
            FieldGroup::Font,
            FieldGroup::Layout,
            FieldGroup::Layer,
            FieldGroup::Cell,
        ]
    );
    for text in ["", ",", "basic,", ",basic", "basic,,font", "Basic", "bogus", "basic layout"] {
        assert!(FieldSet::parse(text).is_err(), "{text:?} is not a field set");
    }
    assert!(FieldSet::parse("basic,basic").is_err(), "a repeated group is a usage error");
    for text in ["", "basic,bogus", "layer,layer"] {
        rejects(&["ui", "view", "node", "Notes", HANDLE, "--session", SESSION, "--fields", text]);
    }
}

#[test]
fn layer_traversal_needs_an_explicit_scope_and_find_needs_a_filter() {
    let failure =
        rejects(&["ui", "view", "tree", "Notes", "--session", SESSION, "--kind", "layer"]);
    let details = failure.details.unwrap();
    assert_eq!(details["argument"], "--kind");
    assert_eq!(details["value"], "layer");
    assert_eq!(details["accepted"], json!(["--window", "--root"]));
    rejects(&[
        "ui",
        "view",
        "find",
        "Notes",
        "--session",
        SESSION,
        "--kind",
        "layer",
        "--class",
        "CALayer",
    ]);
    assert!(
        parse(&[
            "ui",
            "view",
            "tree",
            "Notes",
            "--session",
            SESSION,
            "--kind",
            "layer",
            "--root",
            HANDLE,
        ])
        .is_ok()
    );
    assert!(
        parse(&[
            "ui",
            "view",
            "find",
            "Notes",
            "--session",
            SESSION,
            "--kind",
            "layer",
            "--window",
            "7",
            "--identifier",
            "badge",
        ])
        .is_ok()
    );

    let failure = rejects(&["ui", "view", "find", "Notes", "--session", SESSION]);
    let details = failure.details.unwrap();
    assert_eq!(details["accepted"], json!(["--class", "--identifier"]));
    assert_eq!(
        parse(&[
            "ui",
            "view",
            "find",
            "Notes",
            "--session",
            SESSION,
            "--class",
            "NSButton",
            "--identifier",
            "ok",
        ])
        .unwrap(),
        UiRequest::View(ViewRequest::Find {
            selection: selection("Notes"),
            session: session(),
            scope: NativeScope { window: None, root: None },
            kind: TraversalKind::View,
            filters: Filters {
                class: Some("NSButton".to_owned()),
                identifier: Some("ok".to_owned()),
            },
            depth: 3,
            fields: FieldSet::default(),
            limit: 200,
        })
    );
    rejects(&["ui", "view", "find", "Notes", "--session", SESSION, "--class", ""]);
}

#[test]
fn correlate_requires_one_window_and_one_session() {
    assert_eq!(
        parse(&["ui", "correlate", "Notes", "--session", SESSION, "--window", "53510"]).unwrap(),
        UiRequest::Correlate {
            selection: selection("Notes"),
            session: session(),
            window: WindowId(53510),
            depth: 3,
            limit: 200,
        }
    );
    rejects(&["ui", "correlate", "Notes", "--session", SESSION]);
    rejects(&["ui", "correlate", "Notes", "--window", "53510"]);
    rejects(&["ui", "correlate", "Notes", "--session", SESSION, "--window", "7", "--root", HANDLE]);
}

#[test]
fn the_optional_session_does_not_change_an_unbound_request() {
    assert_eq!(
        parse(&["ui", "windows", "Notes"]).unwrap(),
        UiRequest::Windows { selection: selection("Notes"), session: None }
    );
    assert_eq!(
        parse(&["ui", "windows", "Notes", "--session", SESSION]).unwrap(),
        UiRequest::Windows { selection: selection("Notes"), session: Some(session()) }
    );
    assert_eq!(
        parse(&["ui", "ax", "tree", "Notes", "--session", SESSION, "--depth", "2"]).unwrap(),
        UiRequest::AxTree {
            selection: selection("Notes"),
            session: Some(session()),
            window: None,
            depth: Some(2),
            no_menu_bar: false,
        }
    );
    assert_eq!(
        parse(&["ui", "screenshot", "Notes", "--out", "shot.png", "--session", SESSION]).unwrap(),
        UiRequest::Screenshot {
            selection: selection("Notes"),
            session: Some(session()),
            window: None,
            out: std::path::PathBuf::from("shot.png"),
        }
    );
    rejects(&["ui", "windows", "Notes", "--session", "nope"]);
    rejects(&["ui", "ax", "tree", "Notes", "--session"]);
    rejects(&["ui", "session", "preflight", "Notes", "--session", SESSION]);
}
