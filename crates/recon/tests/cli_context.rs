//! What `recon context` reports about the binary that answered. Every test runs the executable,
//! because the report is about the running process — its path, its signature, its responsibility,
//! and the permissions macOS records against it — and none of that is observable in process.

use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;
mod support;

fn recon(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_recon")).args(arguments).output().expect("start recon")
}

/// The report `recon context` printed, held to the contract every success shares: one compact
/// JSON object on standard output, an empty standard error, and exit 0.
fn report(output: Output) -> Value {
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    assert!(stderr.is_empty(), "recon context printed a diagnostic: {stderr}");
    assert_eq!(output.status.code(), Some(0), "recon context should succeed");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 report");
    assert_eq!(stdout.lines().count(), 1, "one JSON object on one line: {stdout}");
    serde_json::from_str(&stdout).expect("one JSON object")
}

fn context() -> Value {
    report(recon(&["context"]))
}

#[test]
fn context_reports_the_version_commit_and_path_of_the_binary_that_answered() {
    let context = context();

    assert_eq!(context["recon"]["version"], env!("CARGO_PKG_VERSION"));
    match context["recon"]["commit"].as_str() {
        Some(commit) => {
            assert!(
                commit.chars().all(|digit| digit.is_ascii_hexdigit() && !digit.is_uppercase()),
                "{commit}"
            );
        }
        None => assert_eq!(context["recon"]["commit"], Value::Null, "a commit or null"),
    }

    let executable = PathBuf::from(env!("CARGO_BIN_EXE_recon"));
    let canonical = executable.canonicalize().expect("the built recon");
    assert_eq!(context["recon"]["executablePath"], canonical.to_str().expect("a UTF-8 path"));
}

#[test]
fn context_reports_the_host_it_was_built_for() {
    let context = context();

    assert_eq!(context["host"]["os"], std::env::consts::OS);
    assert_eq!(context["host"]["architecture"], std::env::consts::ARCH);
}

#[test]
fn context_takes_no_arguments() {
    for arguments in [["context", "--json"], ["context", "please"]] {
        let output = recon(&arguments);

        assert_eq!(output.status.code(), Some(2), "recon {arguments:?} should be a usage error");
        assert!(output.stdout.is_empty(), "recon {arguments:?} should print no report");
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
        let failure: Value = serde_json::from_str(&stderr).expect("one JSON error object");
        support::validate_failure(&failure, &["context"]);
        assert_eq!(failure["error"]["code"], "INVALID_ARGUMENT", "{stderr}");
    }
}

#[cfg(not(target_os = "macos"))]
#[test]
fn a_host_that_supports_nothing_is_an_answer_rather_than_a_failure() {
    let context = context();

    assert_eq!(context["host"]["version"], Value::Null);
    assert_eq!(context["macos"], Value::Null);
}

#[cfg(target_os = "macos")]
mod macos {
    use std::path::PathBuf;
    use std::process::Command;

    use serde_json::{Value, json};

    use super::{context, report};

    const SCREEN_RECORDING_PANE: &str =
        "System Settings → Privacy & Security → Screen & System Audio Recording";
    const ACCESSIBILITY_PANE: &str = "System Settings → Privacy & Security → Accessibility";

    fn entries<'a>(context: &'a Value, member: &str) -> &'a Vec<Value> {
        context["macos"][member].as_array().unwrap_or_else(|| panic!("an array of {member}"))
    }

    #[test]
    fn the_mac_reports_its_product_version() {
        let context = context();

        let version = context["host"]["version"].as_str().expect("a macOS product version");
        let numbers: Vec<&str> = version.split('.').collect();
        assert_eq!(numbers.len(), 3, "major.minor.patch: {version}");
        assert!(
            numbers
                .iter()
                .all(|number| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())),
            "{version}"
        );
    }

    #[test]
    fn the_signing_identity_of_the_running_binary_is_reported() {
        let signing = context()["macos"]["codeSigning"].clone();

        let signed = signing["signed"].as_bool().expect("whether recon is signed");
        let ad_hoc = signing["adHoc"].as_bool().expect("whether the signature is ad-hoc");
        assert_eq!(signed, signing["identifier"].is_string(), "{signing}");
        if ad_hoc {
            assert!(signed, "an ad-hoc signature is a signature: {signing}");
            assert_eq!(signing["teamIdentifier"], Value::Null, "{signing}");
        }
        // The Rust toolchain ad-hoc signs every Apple silicon binary it links, so the `cargo`
        // build this test runs is the ad-hoc case the report has to recognize.
        #[cfg(target_arch = "aarch64")]
        assert!(ad_hoc, "a cargo-built recon is ad-hoc signed: {signing}");
    }

    #[test]
    fn permissions_are_reported_for_recons_own_identity_in_a_fixed_order() {
        let context = context();
        let responsible =
            context["macos"]["responsibility"]["responsibleForSelf"].as_bool().expect("a flag");
        let permissions = entries(&context, "permissions");

        assert_eq!(permissions.len(), 2, "{permissions:?}");
        assert_eq!(permissions[0]["permission"], "screen-recording");
        assert_eq!(permissions[0]["settingsPane"], SCREEN_RECORDING_PANE);
        assert_eq!(permissions[1]["permission"], "accessibility");
        assert_eq!(permissions[1]["settingsPane"], ACCESSIBILITY_PANE);
        for permission in permissions {
            // A preflight run under the launcher's identity answers a different question, so an
            // unknown grant is null rather than false.
            assert_eq!(
                permission["granted"].is_null(),
                !responsible,
                "granted is null exactly when recon is not responsible for itself: {permission}"
            );
            assert!(permission["granted"].is_boolean() || permission["granted"].is_null());
        }
    }

    #[test]
    fn a_failed_self_disclaim_is_reported_rather_than_failing_the_command() {
        // The marker names the process that spawned this recon, which is this test: recon reads
        // it as "you already re-executed" and reports the state it is actually in. A process that
        // has not disclaimed is never responsible for itself, so this is the fallback path.
        let output = Command::new(env!("CARGO_BIN_EXE_recon"))
            .arg("context")
            .env("RECON_RESPONSIBILITY_DISCLAIMED", std::process::id().to_string())
            .output()
            .expect("start recon");
        let context = report(output);

        let responsibility = context["macos"]["responsibility"].clone();
        assert_eq!(responsibility["responsibleForSelf"], json!(false), "{responsibility}");
        let detail = responsibility["detail"].as_str().expect("why recon is not responsible");
        assert!(detail.contains("responsible"), "{detail}");
        for permission in entries(&context, "permissions") {
            assert_eq!(permission["granted"], Value::Null, "{permission}");
        }
    }

    #[test]
    fn application_discovery_reports_recons_own_spotlight_path() {
        let discovery = context()["macos"]["applicationDiscovery"].clone();

        assert_eq!(discovery["mechanism"], "spotlight");
        let available = discovery["available"].as_bool().expect("whether discovery works");
        match discovery["indexedApplications"].as_u64() {
            Some(count) => {
                assert!(available, "a count means discovery works: {discovery}");
                assert!(count > 0, "{discovery}");
            }
            None => {
                assert!(!available, "no count means discovery does not work: {discovery}");
                assert_eq!(discovery["indexedApplications"], Value::Null, "{discovery}");
            }
        }
    }

    /// The report of the signed executable, which is the only one whose permissions a person can
    /// grant. Opt-in for the same reason as the other live-Mac tests: macOS grants Screen
    /// Recording and Accessibility to one executable at one path, which CI cannot have.
    #[test]
    #[ignore = "needs target/signed/recon with Screen Recording and Accessibility granted; run vp run test:ui"]
    fn the_signed_recon_reports_its_grants_identity_and_interfaces() {
        let signed = signed_recon();

        let context = report(Command::new(&signed).arg("context").output().expect("start recon"));

        assert_eq!(context["recon"]["executablePath"], signed.to_str().expect("a UTF-8 path"));
        let signing = context["macos"]["codeSigning"].clone();
        assert_eq!(signing["signed"], json!(true), "{signing}");
        assert_eq!(signing["identifier"], "me.malstrom.recon", "{signing}");
        assert_eq!(signing["adHoc"], json!(false), "a Developer ID signature: {signing}");
        assert!(signing["teamIdentifier"].is_string(), "{signing}");

        let responsibility = context["macos"]["responsibility"].clone();
        assert_eq!(responsibility["responsibleForSelf"], json!(true), "{responsibility}");
        assert_eq!(responsibility["detail"], Value::Null, "{responsibility}");

        for permission in entries(&context, "permissions") {
            assert_eq!(
                permission["granted"],
                json!(true),
                "grant this executable {} in {}",
                permission["permission"],
                permission["settingsPane"]
            );
        }
        for interface in entries(&context, "interfaces") {
            assert_eq!(interface["available"], json!(true), "{interface}");
        }
        assert_eq!(context["macos"]["applicationDiscovery"]["available"], json!(true));
    }

    /// `recon context` reads state, so running it twice must report the same thing: nothing it
    /// does changes what the next run sees, and nothing it does can show a dialog.
    #[test]
    fn reading_the_context_changes_nothing_it_reports() {
        let first = context();

        let second = context();

        assert_eq!(first["macos"]["permissions"], second["macos"]["permissions"]);
        assert_eq!(first["macos"]["responsibility"], second["macos"]["responsibility"]);
        assert_eq!(first["macos"]["codeSigning"], second["macos"]["codeSigning"]);
    }

    fn signed_recon() -> PathBuf {
        let recon = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/signed/recon")
            .canonicalize()
            .expect("target/signed/recon is missing; build it with `vp run recon:sign`");
        assert!(recon.is_file(), "{} is not a file", recon.display());
        recon
    }
}
