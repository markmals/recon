#![cfg(target_os = "macos")]

use proptest::prelude::*;
use proptest::test_runner::RngSeed;
use serde_json::{Value, json};

mod signing;
mod support;

use signing::{Invocation, ORIGINAL, Probe};

fn context(invocation: &Invocation) -> Value {
    let output = &invocation.output;
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let specification =
        serde_json::to_value(recon::cli::schema::describe(&["context".to_owned()]).unwrap())
            .unwrap();
    assert!(jsonschema::draft202012::is_valid(&specification["command"]["output"], &report));
    report
}

fn internal(invocation: &Invocation) {
    let output = &invocation.output;
    let report = serde_json::from_slice::<Value>(&output.stdout).ok();
    assert_eq!(
        output.status.code(),
        Some(1),
        "signing facts: {:?}; stderr: {}",
        report.as_ref().map(|report| &report["macos"]["codeSigning"]),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "{output:?}");
    assert_eq!(output.stderr.last(), Some(&b'\n'));
    assert_eq!(output.stderr.iter().filter(|byte| **byte == b'\n').count(), 1);
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    support::validate_failure(&error, &["context"]);
    assert_eq!(error["error"]["code"], "INTERNAL", "{error}");
}

fn original_or_internal(invocation: &Invocation) {
    if invocation.output.status.code() == Some(1) {
        internal(invocation);
    } else {
        assert_eq!(
            context(invocation)["macos"]["codeSigning"],
            json!({"signed":true,"identifier":ORIGINAL,"teamIdentifier":null,"adHoc":true})
        );
    }
}

fn own_read_only_queries(invocation: &Invocation) {
    for query in &invocation.queries {
        assert_eq!(query.pid, query.own_pid, "only the executing process may be queried");
        match query.operation {
            0 => assert_eq!(query.length, 4),
            5 => assert_eq!(query.length, 20),
            operation => panic!("unexpected private signing operation {operation}"),
        }
    }
}

#[test]
fn proposal0003_running_signature_is_not_replaced_by_another_signed_file() {
    let invocation = Probe::new().replace(false, "normal");
    original_or_internal(&invocation);
    own_read_only_queries(&invocation);
}

#[test]
fn proposal0003_running_signed_image_never_becomes_unsigned_from_disk_replacement() {
    let invocation = Probe::new().replace(true, "normal");
    original_or_internal(&invocation);
    own_read_only_queries(&invocation);
}

#[test]
fn proposal0003_an_alternate_hash_cannot_bind_different_selected_metadata() {
    let invocation = Probe::new().replace(false, "matching-alternate");
    original_or_internal(&invocation);
    own_read_only_queries(&invocation);
}

fn query_failure(mode: &str, operation: u32) {
    let invocation = Probe::new().run(mode, &[]);
    internal(&invocation);
    assert_eq!(invocation.queries.last().map(|query| query.operation), Some(operation));
    own_read_only_queries(&invocation);
}

#[test]
fn proposal0003_failed_signing_status_query_is_internal() {
    query_failure("status-eio", 0);
}

#[test]
fn proposal0003_failed_signing_hash_query_is_internal() {
    query_failure("hash-eio", 5);
}

#[test]
fn proposal0003_signed_status_with_ebadexec_is_not_unsigned() {
    query_failure("hash-ebadexec", 5);
}

#[test]
fn proposal0003_an_unavailable_required_signing_query_is_internal() {
    internal(&Probe::new().run("missing-query", &[]));
}

#[test]
fn proposal0003_signed_status_cannot_turn_a_public_unsigned_error_into_unsigned() {
    let invocation = Probe::new().run("public-unsigned-error", &[]);
    internal(&invocation);
    own_read_only_queries(&invocation);
}

fn unreadable_metadata(mode: &str) {
    let invocation = Probe::new().run(mode, &[]);
    internal(&invocation);
    own_read_only_queries(&invocation);
}

#[test]
fn proposal0003_selected_signing_hash_is_required() {
    unreadable_metadata("missing-unique");
}

#[test]
fn proposal0003_selected_signing_hash_must_be_data() {
    unreadable_metadata("invalid-unique");
}

#[test]
fn proposal0003_signed_identifier_is_required() {
    unreadable_metadata("missing-identifier");
}

#[test]
fn proposal0003_signed_flags_are_required() {
    unreadable_metadata("missing-flags");
}

#[test]
fn proposal0003_present_team_identifier_must_be_a_string() {
    unreadable_metadata("invalid-team");
}

#[test]
fn proposal0003_positive_unsigned_status_precedes_unreadable_file_metadata() {
    // A synthetic native status exercises precedence, not a genuinely unsigned ARM executable.
    let invocation = Probe::new().run("unsigned-status", &[]);
    assert_eq!(
        context(&invocation)["macos"]["codeSigning"],
        json!({"signed":false,"identifier":null,"teamIdentifier":null,"adHoc":false})
    );
    assert_eq!(invocation.queries.last().map(|query| query.operation), Some(0));
    own_read_only_queries(&invocation);
}

#[test]
fn proposal0003_signature_presence_does_not_require_dynamic_validity_or_trust() {
    let invocation = Probe::new().run("invalid-dynamic-status", &[]);
    assert_eq!(
        context(&invocation)["macos"]["codeSigning"],
        json!({"signed":true,"identifier":ORIGINAL,"teamIdentifier":null,"adHoc":true})
    );
    own_read_only_queries(&invocation);
}

#[test]
fn proposal0003_other_missing_private_interfaces_remain_reportable() {
    let invocation = Probe::new().run("missing-window-server", &[]);
    let report = context(&invocation);
    let interfaces = report["macos"]["interfaces"].as_array().unwrap();
    assert_eq!(
        interfaces.iter().find(|entry| entry["interface"] == "window-server-connection").unwrap(),
        &json!({"interface":"window-server-connection","symbol":"CGSMainConnectionID","available":false})
    );
}

#[test]
fn proposal0003_context_advertises_the_required_self_signing_capability() {
    let invocation = Probe::new().run("normal", &[]);
    let report = context(&invocation);
    let interfaces = report["macos"]["interfaces"].as_array().unwrap();
    assert_eq!(
        interfaces.iter().find(|entry| entry["interface"] == "code-signing-query"),
        Some(&json!({"interface":"code-signing-query","symbol":"csops","available":true}))
    );
    own_read_only_queries(&invocation);
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 8,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(0x0003_2026),
        ..ProptestConfig::default()
    })]

    #[test]
    fn proposal0003_every_changed_selected_hash_bit_breaks_binding(bit in 0usize..160) {
        let invocation = Probe::new().run(
            "normal", &[("RECON_SIGNING_TEST_FLIPPED_BIT", bit.to_string())],
        );
        internal(&invocation);
        own_read_only_queries(&invocation);
    }

    #[test]
    fn proposal0003_only_a_twenty_byte_selected_hash_can_bind(
        length in (0usize..65).prop_filter("not the kernel hash length", |length| *length != 20),
    ) {
        let invocation = Probe::new().run(
            "normal", &[("RECON_SIGNING_TEST_HASH_LENGTH", length.to_string())],
        );
        internal(&invocation);
        own_read_only_queries(&invocation);
    }
}
