//! A missing privacy permission as an error object: which permissions are missing, which
//! executable macOS records them against, and whether its signature survives a rebuild. The
//! recovery data is structured, so nothing here matches on the English sentence.

use std::path::Path;

use recon_core::error::{ErrorCode, ErrorReport};
use recon_core::schema::output_schema;
use recon_privacy_macos::permission::{Permission, permission_denied};
use serde_json::json;

/// Every failure a command reports is one envelope of the published shape.
fn validate_failure(failure: &recon_core::error::Failure) {
    let schema = output_schema::<ErrorReport<'static>>();
    let validator = jsonschema::draft202012::new(&schema).expect("the envelope schema compiles");
    let envelope = serde_json::to_value(failure.report()).expect("the envelope encodes");
    let errors =
        validator.iter_errors(&envelope).map(|error| error.to_string()).collect::<Vec<_>>();
    assert!(errors.is_empty(), "{errors:?}: {envelope}");
}

const EXECUTABLE: &str = "/usr/local/bin/recon";

const SCREEN_RECORDING_PANE: &str =
    "System Settings → Privacy & Security → Screen & System Audio Recording";
const ACCESSIBILITY_PANE: &str = "System Settings → Privacy & Security → Accessibility";

#[test]
fn a_missing_permission_carries_the_executable_and_the_settings_pane() {
    let failure = permission_denied(&[Permission::ScreenRecording], Path::new(EXECUTABLE), false);

    validate_failure(&failure);
    assert_eq!(failure.code, ErrorCode::PermissionDenied);
    assert!(!failure.retryable, "a person must grant it first");
    assert_eq!(failure.exit_code(), 1);
    assert_eq!(
        failure.details,
        Some(json!({
            "permissions": [
                {"permission": "screen-recording", "settingsPane": SCREEN_RECORDING_PANE},
            ],
            "executablePath": EXECUTABLE,
            "adHocSigned": false,
        }))
    );
}

#[test]
fn accessibility_carries_its_own_settings_pane() {
    let failure = permission_denied(&[Permission::Accessibility], Path::new(EXECUTABLE), false);

    validate_failure(&failure);
    assert_eq!(
        failure.details.expect("details")["permissions"],
        json!([{"permission": "accessibility", "settingsPane": ACCESSIBILITY_PANE}])
    );
}

#[test]
fn both_missing_permissions_are_listed_in_order() {
    let failure = permission_denied(
        &[Permission::ScreenRecording, Permission::Accessibility],
        Path::new(EXECUTABLE),
        false,
    );
    validate_failure(&failure);

    assert_eq!(
        failure.details.expect("details")["permissions"],
        json!([
            {"permission": "screen-recording", "settingsPane": SCREEN_RECORDING_PANE},
            {"permission": "accessibility", "settingsPane": ACCESSIBILITY_PANE},
        ])
    );
}

/// An ad-hoc signature's designated requirement is the code hash itself, so macOS forgets the
/// grant on the next rebuild. That is a fact about the binary, so it is a field an agent can read
/// rather than a sentence it would have to parse.
#[test]
fn an_ad_hoc_signature_is_reported_as_a_field() {
    let failure = permission_denied(&[Permission::Accessibility], Path::new(EXECUTABLE), true);
    validate_failure(&failure);

    assert_eq!(failure.details.expect("details")["adHocSigned"], json!(true));
}

#[test]
fn every_permission_has_a_stable_identifier_and_a_settings_pane() {
    assert_eq!(
        Permission::ALL.map(Permission::identifier),
        ["screen-recording", "accessibility"],
        "the identifiers recon schema and recon context publish"
    );
    assert_eq!(
        Permission::ALL.map(Permission::settings_pane),
        [SCREEN_RECORDING_PANE, ACCESSIBILITY_PANE]
    );
}
