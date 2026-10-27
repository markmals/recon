//! What `recon app list` reports with and without `--running`. The listing is built from
//! discovered applications, so these tests need no Mac, no Spotlight index, and no permission.

use std::path::PathBuf;

use proptest::prelude::*;
use recon::application::response::ListResponse;
use recon::application::{Application, Pid, normalize_applications};
use serde_json::Value;

fn application(
    name: &str,
    bundle_id: Option<&str>,
    path: &str,
    running_pids: &[u32],
) -> Application {
    Application {
        name: name.to_owned(),
        bundle_id: bundle_id.map(str::to_owned),
        path: PathBuf::from(path),
        version: None,
        build: None,
        executable_path: None,
        running_pids: running_pids.iter().copied().map(Pid).collect(),
    }
}

/// Four bundles as discovery leaves them: two with a running process, one without a bundle
/// identifier, and both running entries observed out of order.
fn discovered() -> Vec<Application> {
    normalize_applications(vec![
        application("Notes", Some("com.apple.Notes"), "/System/Applications/Notes.app", &[412, 77]),
        application("Chess", Some("com.apple.Chess"), "/System/Applications/Chess.app", &[]),
        application("Terminal", None, "/System/Applications/Utilities/Terminal.app", &[901]),
        application("Safari", Some("com.apple.Safari"), "/Applications/Safari.app", &[]),
    ])
}

fn listed(applications: &[Application], running: bool) -> Value {
    serde_json::to_value(ListResponse::new(applications, running)).expect("serialize the listing")
}

fn entries(listing: &Value) -> &Vec<Value> {
    listing["applications"].as_array().expect("an applications array")
}

fn names(listing: &Value) -> Vec<&str> {
    entries(listing).iter().map(|entry| entry["name"].as_str().expect("a listed name")).collect()
}

#[test]
fn proposal_0004_running_lists_exactly_the_applications_with_a_running_process() {
    let discovered = discovered();

    let listing = listed(&discovered, true);

    assert_eq!(names(&listing), ["Notes", "Terminal"]);
    assert_eq!(listing["discovery"], "running");
}

#[test]
fn proposal_0004_running_keeps_the_fields_order_and_process_ids_of_the_full_list() {
    let discovered = discovered();

    let listing = listed(&discovered, true);

    assert_eq!(entries(&listing)[0]["runningPids"], serde_json::json!([77, 412]));
    assert_eq!(entries(&listing)[1]["runningPids"], serde_json::json!([901]));
    assert_eq!(entries(&listing)[1]["bundleId"], Value::Null);
    assert!(entries(&listing)[1].get("bundleId").is_some());
}

#[test]
fn proposal_0004_running_reports_an_empty_list_rather_than_an_error() {
    let discovered = normalize_applications(vec![application(
        "Chess",
        Some("com.apple.Chess"),
        "/System/Applications/Chess.app",
        &[],
    )]);

    let listing = listed(&discovered, true);

    assert_eq!(listing["discovery"], "running");
    assert_eq!(listing["applications"], serde_json::json!([]));
}

#[test]
fn proposal_0004_a_list_without_running_is_unchanged_including_its_nulls() {
    let discovered = discovered();

    let listing = listed(&discovered, false);

    assert_eq!(listing["discovery"], "indexed-and-running");
    assert_eq!(names(&listing), ["Chess", "Notes", "Safari", "Terminal"]);
    assert_eq!(entries(&listing)[0]["runningPids"], serde_json::json!([]));
    assert!(entries(&listing)[3].get("bundleId").is_some());
    assert_eq!(entries(&listing)[3]["bundleId"], Value::Null);
}

proptest! {
    /// `--running` holds exactly the entries of the unfiltered list whose `runningPids` is not
    /// empty, with the same fields, the same order, and the same process IDs.
    #[test]
    fn proposal_0004_running_holds_exactly_the_running_entries_of_the_full_list(
        observations in prop::collection::vec(
            (any::<u8>(), any::<u8>(), prop::collection::vec(1u32..4096, 0..4)),
            0..24,
        ),
    ) {
        let discovered = normalize_applications(
            observations
                .iter()
                .enumerate()
                .map(|(index, (name, volume, pids))| {
                    application(
                        &format!("App {name}"),
                        None,
                        &format!("/Volumes/{volume}/{index}/App.app"),
                        pids,
                    )
                })
                .collect(),
        );

        let full = listed(&discovered, false);
        let running = listed(&discovered, true);

        let expected: Vec<&Value> = entries(&full)
            .iter()
            .filter(|entry| !entry["runningPids"].as_array().expect("process IDs").is_empty())
            .collect();

        prop_assert_eq!(entries(&running).iter().collect::<Vec<&Value>>(), expected);
    }
}
