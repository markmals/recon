//! Recon on macOS: finding applications through Spotlight and the running workspace, and
//! observing them through Accessibility and the window server.

pub mod inject;
mod investigation;
mod metadata;
mod ui;
mod workspace;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::{Command, Output};

use serde::Serialize;
use serde_json::json;

use crate::application::{Application, TargetError, normalize_applications, resolve_target};
use crate::error::{ErrorCode, Failure, NotFound};

pub use investigation::investigate;
pub use ui::context::{facts as macos_facts, product_version};
pub use ui::observe;

/// The one Spotlight query Recon discovers applications with.
const INDEXED_APPLICATIONS: &str = "kMDItemContentTypeTree == 'com.apple.application-bundle'";

/// The readable application bundles that the Spotlight index lists or that running processes were
/// launched from.
fn applications() -> Result<Vec<Application>, Failure> {
    let output = search_index()?;
    // The listing promises only bundles it can read, so an indexed or running path that is not a
    // readable application bundle is left out rather than failing the whole listing.
    let mut discovered: Vec<Application> = indexed_paths(checked_indexed_output(&output)?)
        .filter_map(|path| metadata::inspect_bundle(path).ok())
        .collect();
    for (path, pid) in workspace::running_applications() {
        if let Some(application) = discovered.iter_mut().find(|app| app.path == path) {
            application.running_pids.push(pid);
        } else if let Ok(mut application) = metadata::inspect_bundle(&path) {
            application.running_pids.push(pid);
            discovered.push(application);
        }
    }
    Ok(normalize_applications(discovered))
}

/// How many application bundles the Spotlight query behind `app list` reports, counted rather
/// than listed.
///
/// This is Recon's own discovery path, which is what `recon context` reports on. It reads the
/// index, asks for no permission, and shows nothing on screen.
///
/// # Errors
///
/// [`ErrorCode::DiscoveryUnavailable`] in exactly the three conditions under which `app list`
/// fails: the query cannot be run, it exits unsuccessfully, or it reports nothing indexed.
pub fn indexed_application_count() -> Result<usize, Failure> {
    let output = search_index()?;
    Ok(indexed_paths(checked_indexed_output(&output)?).count())
}

fn search_index() -> Result<Output, Failure> {
    Command::new("/usr/bin/mdfind").args(["-0", INDEXED_APPLICATIONS]).output().map_err(|error| {
        discovery_unavailable(
            format!("cannot search indexed applications: {error}"),
            Some(error.to_string()),
        )
    })
}

fn indexed_paths(output: &[u8]) -> impl Iterator<Item = &Path> {
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| Path::new(OsStr::from_bytes(path)))
}

fn checked_indexed_output(output: &Output) -> Result<&[u8], Failure> {
    if !output.status.success() {
        let reported = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(discovery_unavailable(
            format!("cannot search indexed applications: {reported}"),
            (!reported.is_empty()).then_some(reported),
        ));
    }
    if output.stdout.iter().all(|byte| *byte == 0) {
        return Err(discovery_unavailable(
            "cannot search indexed applications: no indexed applications found; check Spotlight \
             indexing"
                .to_owned(),
            Some("no indexed applications found".to_owned()),
        ));
    }
    Ok(&output.stdout)
}

fn discovery_unavailable(message: String, detail: Option<String>) -> Failure {
    Failure::new(
        ErrorCode::DiscoveryUnavailable,
        message,
        json!({ "mechanism": "spotlight", "detail": detail }),
    )
}

/// Resolves a selector and runs `use_application` on the application it names.
///
/// An absolute path is inspected directly rather than looked up among discovered applications, so
/// it can name a bundle the index does not list.
fn with_target<T>(
    selector: &str,
    use_application: impl FnOnce(&Application) -> Result<T, Failure>,
) -> Result<T, Failure> {
    if Path::new(selector).is_absolute() {
        let mut application = metadata::inspect_bundle(Path::new(selector))?;
        application.running_pids = workspace::running_applications()
            .into_iter()
            .filter_map(|(path, pid)| (path == application.path).then_some(pid))
            .collect();
        application.normalize_running_pids();
        return use_application(&application);
    }

    let applications = applications()?;
    let application = resolve_target(&applications, selector).map_err(target_failure)?;
    use_application(application)
}

/// Why a selector names no single application, as data rather than as a list in prose.
fn target_failure(error: TargetError<'_>) -> Failure {
    match error {
        TargetError::NotFound { selector } => Failure::new(
            ErrorCode::NotFound,
            format!("no application matches {selector:?}"),
            NotFound { kind: "application", selector: Some(selector), ..Default::default() }
                .details(),
        ),
        TargetError::Ambiguous { selector, candidates } => Failure::new(
            ErrorCode::AmbiguousTarget,
            format!("ambiguous application {selector:?}; choose a bundle path"),
            json!({
                "selector": selector,
                "candidates": candidates
                    .iter()
                    .map(|candidate| json!({
                        "name": candidate.name,
                        "bundleId": candidate.bundle_id,
                        "path": candidate.path.display().to_string(),
                    }))
                    .collect::<Vec<_>>(),
            }),
        ),
    }
}

fn encode(response: &impl Serialize) -> Result<String, Failure> {
    serde_json::to_string(response)
        .map_err(|error| Failure::internal(format!("cannot encode response: {error}")))
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::path::PathBuf;
    use std::process::{ExitStatus, Output};

    use serde_json::{Value, json};

    use super::*;

    fn details(failure: &Failure) -> Value {
        failure.details.clone().expect("the data needed to recover")
    }

    fn application(name: &str, bundle_id: Option<&str>, path: &str) -> Application {
        Application {
            name: name.to_owned(),
            bundle_id: bundle_id.map(str::to_owned),
            path: PathBuf::from(path),
            version: None,
            build: None,
            executable_path: None,
            running_pids: Vec::new(),
        }
    }

    #[test]
    fn an_empty_index_does_not_claim_indexed_discovery() {
        let output =
            Output { status: ExitStatus::from_raw(0), stdout: Vec::new(), stderr: Vec::new() };

        let failure = checked_indexed_output(&output).unwrap_err();

        assert_eq!(failure.code, ErrorCode::DiscoveryUnavailable);
        assert!(failure.retryable, "another attempt can find an index that is being rebuilt");
        assert_eq!(
            details(&failure),
            json!({ "mechanism": "spotlight", "detail": "no indexed applications found" })
        );
    }

    #[test]
    fn a_failed_index_query_reports_what_spotlight_said() {
        let output = Output {
            status: ExitStatus::from_raw(1 << 8),
            stdout: Vec::new(),
            stderr: b"metadata server unavailable".to_vec(),
        };

        let failure = checked_indexed_output(&output).unwrap_err();

        assert_eq!(failure.code, ErrorCode::DiscoveryUnavailable);
        assert_eq!(
            details(&failure),
            json!({ "mechanism": "spotlight", "detail": "metadata server unavailable" })
        );
    }

    #[test]
    fn an_unmatched_selector_is_not_found_and_names_the_selector() {
        let applications = [application("Logic Pro", None, "/Applications/Logic Pro.app")];

        let failure = target_failure(resolve_target(&applications, "Logic").unwrap_err());

        assert_eq!(failure.code, ErrorCode::NotFound);
        assert_eq!(
            details(&failure),
            json!({
                "kind": "application",
                "selector": "Logic",
                "path": null,
                "pid": null,
                "windowId": null,
                "runningPids": null,
                "detail": null,
            })
        );
    }

    #[test]
    fn an_ambiguous_selector_carries_every_candidate_as_data() {
        let applications = [
            application("Contacts", Some("com.apple.AddressBook"), "/System/Contacts.app"),
            application("Contacts", None, "/Users/test/Contacts.app"),
        ];

        let failure = target_failure(resolve_target(&applications, "Contacts").unwrap_err());

        assert_eq!(failure.code, ErrorCode::AmbiguousTarget);
        assert_eq!(
            details(&failure),
            json!({
                "selector": "Contacts",
                "candidates": [
                    {
                        "name": "Contacts",
                        "bundleId": "com.apple.AddressBook",
                        "path": "/System/Contacts.app",
                    },
                    { "name": "Contacts", "bundleId": null, "path": "/Users/test/Contacts.app" },
                ],
            })
        );
    }
}
