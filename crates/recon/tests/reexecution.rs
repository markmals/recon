#![cfg(target_os = "macos")]

use std::io::Read;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::LazyLock;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

static PROBE: LazyLock<tempfile::TempDir> = LazyLock::new(|| {
    let scratch = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reexecution_probe.c");
    let status = Command::new("cc")
        .arg("-dynamiclib")
        .arg(source)
        .arg("-o")
        .arg(scratch.path().join("probe.dylib"))
        .status()
        .unwrap();
    assert!(status.success());
    scratch
});

fn probe(arguments: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_recon"));
    command
        .args(arguments)
        .env("DYLD_INSERT_LIBRARIES", PROBE.path().join("probe.dylib"))
        .env_remove("RECON_RESPONSIBILITY_DISCLAIMED")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    command
}

fn run(arguments: &[&str], failed_wait: bool, large: bool) -> Output {
    let mut command = probe(arguments);
    if failed_wait {
        command.env("RECON_TEST_FAIL_WAIT", "1");
    }
    if large {
        command.env("RECON_TEST_LARGE_CHILD_OUTPUT", "1");
    }
    capture(&mut command)
}

fn capture(command: &mut Command) -> Output {
    let mut child = command.spawn().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let stdout = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let stderr = thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            // The owned child is still unreaped, so its process group cannot have been reused.
            // SAFETY: only this test's isolated subprocess group is terminated on timeout.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
            panic!("re-execution deadlocked while draining child output");
        }
        thread::sleep(Duration::from_millis(10));
    };
    Output { status, stdout: stdout.join().unwrap(), stderr: stderr.join().unwrap() }
}

fn wait_failure(output: &Output, command: &[&str]) {
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout bytes: {}; stderr bytes: {}",
        output.stdout.len(),
        output.stderr.len()
    );
    assert!(output.stdout.is_empty(), "{} stdout bytes leaked", output.stdout.len());
    assert_eq!(output.stderr.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert_eq!(output.stderr.last(), Some(&b'\n'));
    let envelope: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(envelope["error"]["code"], "IDENTITY_UNAVAILABLE");
    assert_eq!(envelope["error"]["details"]["stage"], "wait");
    assert_eq!(envelope["error"]["command"], serde_json::json!(command));
    assert_eq!(envelope["error"]["retryable"], false);
    let schema = recon::cli::schema::output_schema::<recon::error::ErrorReport<'static>>();
    assert!(jsonschema::draft202012::is_valid(&schema, &envelope));
    let registry = recon::cli::registry::registry();
    let entry = registry.iter().find(|entry| entry.path == command).unwrap();
    assert!(
        entry
            .errors
            .iter()
            .any(|code| serde_json::to_value(code).unwrap() == envelope["error"]["code"])
    );
}

#[test]
fn proposal0003_wait_failure_replaces_a_real_child_error() {
    wait_failure(&run(&["ui", "windows", "/recon-no-such.app"], true, false), &["ui", "windows"]);
}

#[test]
fn proposal0003_context_wait_failure_discards_success_and_never_falls_back() {
    wait_failure(&run(&["context"], true, false), &["context"]);
}

#[test]
fn proposal0003_wait_failure_discards_both_streams_larger_than_pipe_buffers() {
    wait_failure(&run(&["context"], true, true), &["context"]);
}

#[test]
fn proposal0003_successful_wait_preserves_large_streams_and_child_status() {
    let output = run(&["context"], false, true);
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, vec![b'o'; 1_048_576]);
    assert_eq!(output.stderr, vec![b'e'; 1_048_576]);
}

#[test]
fn proposal0003_context_schema_advertises_post_spawn_wait_failure() {
    let output =
        Command::new(env!("CARGO_BIN_EXE_recon")).args(["schema", "context"]).output().unwrap();
    assert!(output.status.success());
    let specification: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        specification["command"]["errors"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("IDENTITY_UNAVAILABLE"))
    );
}

#[test]
fn proposal0003_a_signalled_child_is_reraised_after_stream_collection() {
    let mut command = probe(&["context"]);
    command.env("RECON_TEST_CHILD_SIGNAL", "1");
    let output = capture(&mut command);
    assert_eq!(output.status.signal(), Some(libc::SIGTERM));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

/// Runs `recon context` with the probe refusing to start reader number `reader` of the child's
/// output, and reports whether a child was spawned. The probe marks the scratch path from inside
/// the parent's own spawn call, once that call has really produced a child, and the child it
/// produces exits before recon's startup reaches anything. The marker is therefore a trace of
/// the re-execution itself, recorded before the parent can exit, and never of the `mdfind` the
/// context fallback legitimately runs.
fn spawn_after_refused_reader(reader: &str) -> (Output, bool) {
    let scratch = tempfile::tempdir().unwrap();
    let marker = scratch.path().join("child-spawned");
    let mut command = probe(&["context"]);
    command.env("RECON_TEST_READER_FAILURE", reader).env("RECON_TEST_CHILD_MARKER", &marker);
    let output = capture(&mut command);
    (output, marker.exists())
}

/// An identity failure before any child exists is reportable: `context` produces its report, says
/// recon is not responsible for itself and why, and leaves every grant unknown rather than
/// preflighting under the launcher's identity.
fn unknown_grants(output: &Output) {
    assert!(
        output.stderr.is_empty(),
        "recon context printed a diagnostic: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let specification =
        serde_json::to_value(recon::cli::schema::describe(&["context".to_owned()]).unwrap())
            .unwrap();
    assert!(
        jsonschema::draft202012::is_valid(&specification["command"]["output"], &report),
        "{report}"
    );
    let responsibility = &report["macos"]["responsibility"];
    assert_eq!(responsibility["responsibleForSelf"], serde_json::json!(false));
    assert!(responsibility["detail"].is_string(), "{responsibility}");
    let permissions = report["macos"]["permissions"].as_array().unwrap();
    assert_eq!(permissions.len(), 2, "{permissions:?}");
    for permission in permissions {
        assert!(permission["granted"].is_null(), "a grant was preflighted anyway: {permission}");
    }
}

#[test]
fn proposal0003_a_first_reader_that_cannot_start_spawns_no_child() {
    let (output, child_spawned) = spawn_after_refused_reader("1");

    assert!(!child_spawned, "a child was spawned although no reader could drain it");
    unknown_grants(&output);
}

#[test]
fn proposal0003_a_second_reader_that_cannot_start_spawns_no_child() {
    let (output, child_spawned) = spawn_after_refused_reader("2");

    assert!(!child_spawned, "a child was spawned although only one reader could drain it");
    unknown_grants(&output);
}
