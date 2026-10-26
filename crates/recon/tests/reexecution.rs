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
