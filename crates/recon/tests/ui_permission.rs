use std::path::Path;

use recon::ui::permission::{Permission, Signature, missing_permissions_message};

const EXECUTABLE: &str = "/usr/local/bin/recon";

#[test]
fn a_missing_permission_names_the_executable_and_the_settings_pane() {
    let message = missing_permissions_message(
        &[Permission::ScreenRecording],
        Path::new(EXECUTABLE),
        Signature::Stable,
    );
    assert_eq!(
        message,
        "missing permission for /usr/local/bin/recon:\n  Screen Recording: System Settings → \
         Privacy & Security → Screen & System Audio Recording"
    );
}

#[test]
fn accessibility_names_its_own_settings_pane() {
    let message = missing_permissions_message(
        &[Permission::Accessibility],
        Path::new(EXECUTABLE),
        Signature::Stable,
    );
    assert_eq!(
        message,
        "missing permission for /usr/local/bin/recon:\n  Accessibility: System Settings → Privacy \
         & Security → Accessibility"
    );
}

#[test]
fn both_missing_permissions_are_listed_in_order() {
    let message = missing_permissions_message(
        &[Permission::ScreenRecording, Permission::Accessibility],
        Path::new(EXECUTABLE),
        Signature::Stable,
    );
    assert_eq!(
        message,
        "missing permissions for /usr/local/bin/recon:\n  Screen Recording: System Settings → \
         Privacy & Security → Screen & System Audio Recording\n  Accessibility: System Settings → \
         Privacy & Security → Accessibility"
    );
}

#[test]
fn an_ad_hoc_signature_explains_that_rebuilding_forgets_the_grant() {
    let message = missing_permissions_message(
        &[Permission::Accessibility],
        Path::new(EXECUTABLE),
        Signature::AdHoc,
    );
    let rebuilt = message.lines().last().unwrap();
    assert!(rebuilt.contains("ad-hoc signed"), "{message}");
    assert!(rebuilt.contains("rebuilt"), "{message}");
    assert!(rebuilt.contains("Developer ID Application"), "{message}");
    assert!(message.contains("Privacy & Security → Accessibility"), "{message}");
}

#[test]
fn a_stable_signature_does_not_mention_rebuilding() {
    let message = missing_permissions_message(
        &[Permission::ScreenRecording],
        Path::new(EXECUTABLE),
        Signature::Stable,
    );
    assert!(!message.contains("ad-hoc"), "{message}");
    assert!(!message.contains("Developer ID"), "{message}");
}
