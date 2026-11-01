//! Choosing which running instance of a bundle a command observes. The rules are about the
//! process IDs discovery reported, so these tests need no Mac and no running application.

use std::path::Path;

use proptest::prelude::*;
use recon_applications_macos::selection::select_instance;
use recon_core::error::{ErrorCode, ErrorReport, Failure};
use recon_core::process::Pid;
use recon_core::schema::output_schema;
use serde_json::{Value, json};

const BUNDLE: &str = "/Applications/Logic Pro.app";

/// Every failure a command reports is one envelope of the published shape.
fn details(failure: &Failure) -> Value {
    let schema = output_schema::<ErrorReport<'static>>();
    let validator = jsonschema::draft202012::new(&schema).expect("the envelope schema compiles");
    let envelope = serde_json::to_value(failure.report()).expect("the envelope encodes");
    let errors =
        validator.iter_errors(&envelope).map(|error| error.to_string()).collect::<Vec<_>>();
    assert!(errors.is_empty(), "{errors:?}: {envelope}");
    failure.details.clone().expect("the data needed to recover")
}

#[test]
fn the_only_running_instance_is_selected_without_a_pid() {
    assert_eq!(select_instance(Path::new(BUNDLE), &[Pid(412)], None).unwrap(), Pid(412));
}

#[test]
fn a_bundle_that_is_not_running_is_never_launched() {
    for requested in [None, Some(Pid(412))] {
        let failure = select_instance(Path::new(BUNDLE), &[], requested).unwrap_err();

        assert_eq!(failure.code, ErrorCode::NotRunning);
        assert!(!failure.retryable);
        assert_eq!(details(&failure), json!({ "application": BUNDLE }));
    }
}

#[test]
fn several_instances_are_an_ambiguous_instance_naming_every_process() {
    let failure = select_instance(Path::new(BUNDLE), &[Pid(410), Pid(902)], None).unwrap_err();

    assert_eq!(failure.code, ErrorCode::AmbiguousInstance);
    assert!(!failure.retryable);
    assert_eq!(details(&failure), json!({ "application": BUNDLE, "pids": [410, 902] }));
}

#[test]
fn a_requested_pid_selects_that_instance() {
    let running = [Pid(410), Pid(902)];
    assert_eq!(select_instance(Path::new(BUNDLE), &running, Some(Pid(902))).unwrap(), Pid(902));
    assert_eq!(select_instance(Path::new(BUNDLE), &[Pid(410)], Some(Pid(410))).unwrap(), Pid(410));
}

#[test]
fn a_requested_pid_must_be_an_instance_of_the_resolved_bundle() {
    let running = [Pid(410), Pid(902)];

    let failure = select_instance(Path::new(BUNDLE), &running, Some(Pid(411))).unwrap_err();

    assert_eq!(failure.code, ErrorCode::NotFound);
    assert_eq!(
        details(&failure),
        json!({
            "kind": "instance",
            "selector": null,
            "path": BUNDLE,
            "pid": 411,
            "windowId": null,
            "runningPids": [410, 902],
            "detail": null,
        })
    );
}

proptest! {
    #[test]
    fn several_instances_are_never_resolved_without_the_pid_flag(
        pids in prop::collection::hash_set(1u32..100_000, 2..12)
    ) {
        let pids = pids.into_iter().map(Pid).collect::<Vec<_>>();
        let failure = select_instance(Path::new(BUNDLE), &pids, None).unwrap_err();
        prop_assert_eq!(failure.code, ErrorCode::AmbiguousInstance);
        let reported = details(&failure);
        let reported = reported["pids"].as_array().expect("the running process ids");
        prop_assert_eq!(reported.len(), pids.len());
        for pid in &pids {
            prop_assert!(reported.contains(&json!(pid.0)), "{} omits {pid}", details(&failure));
        }
    }

    #[test]
    fn only_a_running_pid_is_ever_selected(
        pids in prop::collection::hash_set(1u32..1_000, 0..8),
        requested in 1u32..1_000,
    ) {
        let pids = pids.into_iter().map(Pid).collect::<Vec<_>>();
        let requested = Pid(requested);
        match select_instance(Path::new(BUNDLE), &pids, Some(requested)) {
            Ok(selected) => {
                prop_assert_eq!(selected, requested);
                prop_assert!(pids.contains(&requested));
            }
            Err(failure) => {
                details(&failure);
                prop_assert!(!pids.contains(&requested));
            }
        }
    }
}
