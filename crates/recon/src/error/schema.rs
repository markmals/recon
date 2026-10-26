use schemars::{Schema, SchemaGenerator};
use serde_json::{Value, json};

pub(super) fn details(generator: &mut SchemaGenerator) -> Schema {
    let string = json!({"type":"string"});
    let optional_string = json!({"type":["string","null"]});
    let integer = json!({"type":"integer","minimum":0});
    let strings = json!({"type":"array","items":string});
    let optional_strings = json!({"type":["array","null"],"items":string});
    let integers = json!({"type":"array","items":integer});
    let candidates = json!({"type":"array","items":object(json!({"name":string,"bundleId":optional_string,"path":string}))});
    let permissions = json!({"type":"array","items":object(json!({"permission":{"enum":["screen-recording","accessibility"]},"settingsPane":string}))});
    let shapes = vec![
        object(
            json!({"argument":optional_string,"value":optional_string,"expected":string,"accepted":optional_strings}),
        ),
        object(json!({"host":string,"supportedHosts":strings})),
        object(json!({"mechanism":{"const":"spotlight"},"detail":optional_string})),
        serde_json::to_value(generator.subschema_for::<super::NotFound<'static>>())
            .expect("not-found schema serializes"),
        object(
            json!({"path":string,"reason":{"enum":["not-a-bundle-directory","unreadable-info-plist","no-name"]},"detail":optional_string}),
        ),
        object(json!({"selector":string,"candidates":candidates})),
        object(json!({"application":string,"pids":integers})),
        object(json!({"application":string})),
        object(
            json!({"permissions":permissions,"executablePath":string,"adHocSigned":{"type":"boolean"}}),
        ),
        object(
            json!({"stage":{"enum":["lookup","spawn","disclaim","wait","verify"]},"symbol":optional_string,"detail":optional_string}),
        ),
        object(json!({"interface":string,"symbol":string})),
        object(json!({"pid":integer,"timeoutSeconds":{"const":2}})),
        object(
            json!({"windowId":integer,"stage":{"enum":["capture","pixels","encode"]},"detail":optional_string}),
        ),
        object(
            json!({"application":string,"reason":{"enum":["executable-missing","path-not-utf8","refused","no-process-id","not-opened","callback-failed"]},"detail":optional_string}),
        ),
        object(json!({"path":string})),
        object(
            json!({"path":optional_string,"operation":{"enum":["resolve-output","write","read-current-directory"]},"reason":optional_string,"detail":string,"partialFile":optional_string}),
        ),
        object(json!({"detail":string})),
        json!({"type":"null"}),
    ];
    serde_json::from_value(json!({"anyOf":shapes})).expect("valid error detail schema")
}

pub(super) fn not_found_kind(_: &mut SchemaGenerator) -> Schema {
    serde_json::from_value(
        json!({"enum":["application","path","instance","window","window-element","main-window"]}),
    )
    .expect("valid not-found kinds")
}

fn object(properties: Value) -> Value {
    let required =
        properties.as_object().expect("properties object").keys().cloned().collect::<Vec<_>>();
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
