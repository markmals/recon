#![cfg(target_os = "macos")]
//! Proposal 0005: the shipped universal C loader binds the requested incarnation, hands the helper
//! its endpoint, and executes the helper inside arm64 and arm64e targets.

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc;
use std::thread::{self, sleep};
use std::time::{Duration, Instant};

use recon_native_access_macos::inject;
use tempfile::TempDir;

fn artifact(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target").join(name)
}

fn fixture_executable() -> PathBuf {
    let binary = artifact("fixtures/inject-fixture");
    std::fs::canonicalize(&binary).unwrap_or(binary)
}

struct Fixture(Child);

impl Fixture {
    fn launch(arm64e: bool) -> Self {
        let binary = artifact("fixtures/inject-fixture");
        let mut command = Command::new("arch");
        command.arg(if arm64e { "-arm64e" } else { "-arm64" }).arg(&binary);
        let mut child = command.stdout(Stdio::piped()).spawn().expect("spawn the inject fixture");
        wait_for_fixture(&mut child);
        Self(child)
    }

    fn pid(&self) -> i32 {
        self.0.id() as i32
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Wait for the fixture to print its pid. `arch` execs the fixture, and exec starts a new
/// incarnation of the same pid, so only an incarnation read after that line is the bindable one.
fn wait_for_fixture(child: &mut Child) {
    let output = child.stdout.take().expect("piped fixture output");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let _ = BufReader::new(output).read_line(&mut line);
        let _ = sender.send(line);
    });
    let line =
        receiver.recv_timeout(Duration::from_secs(5)).expect("the fixture never printed its pid");
    assert_eq!(line.trim().parse::<u32>().ok(), Some(child.id()), "the fixture never started");
}

/// A fresh endpoint path in a directory only this user can reach, as Recon's store provides.
fn private_endpoint() -> (TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("endpoint directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private endpoint directory");
    let endpoint = directory.path().join("sock");
    (directory, endpoint)
}

fn run_loader(pid: i32, pidversion: u32, endpoint: &Path, executable: &Path) -> Output {
    Command::new("arch")
        .arg("-arm64e")
        .arg(artifact("recon-loader"))
        .arg(pid.to_string())
        .arg(pidversion.to_string())
        .arg(artifact("fixtures/inject-helper.dylib"))
        .arg(endpoint)
        .arg(executable)
        .output()
        .expect("run the signed C loader")
}

/// The pid the helper answers with on its endpoint, once it is listening.
fn answering_pid(endpoint: &Path) -> i32 {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stream = loop {
        match UnixStream::connect(endpoint) {
            Ok(stream) => break stream,
            Err(error) => {
                assert!(Instant::now() < deadline, "the helper never listened: {error}");
                sleep(Duration::from_millis(20));
            }
        }
    };
    let mut answer = String::new();
    stream.read_to_string(&mut answer).expect("the helper's answer");
    answer.trim().parse().expect("a pid")
}

#[test]
#[ignore = "requires signed loader and cooperative fixtures; run via `vp run test:inject`"]
fn c_loader_rejects_a_mismatched_executable_before_loading() {
    let mut fixture = Fixture::launch(true);
    let incarnation = inject::identify(fixture.pid()).expect("identify the fixture");
    let (_directory, endpoint) = private_endpoint();

    // The incarnation is correct, but the expected executable names a different binary, so only
    // the loader's executable check can refuse it.
    let output =
        run_loader(fixture.pid(), incarnation.pidversion, &endpoint, Path::new("/usr/bin/true"));

    assert!(!output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!endpoint.exists(), "a mismatched executable was handed an endpoint");
    assert!(fixture.0.try_wait().expect("query fixture").is_none());
}

#[test]
#[ignore = "requires signed loader and cooperative fixtures; run via `vp run test:inject`"]
fn c_loader_executes_inside_an_arm64_target() {
    assert_c_loader_executes(false);
}

#[test]
#[ignore = "requires signed loader and cooperative fixtures; run via `vp run test:inject`"]
fn c_loader_executes_inside_an_arm64e_target() {
    assert_c_loader_executes(true);
}

#[test]
#[ignore = "requires signed loader and cooperative fixtures; run via `vp run test:inject`"]
fn c_loader_rejects_a_reused_incarnation_before_loading() {
    let mut fixture = Fixture::launch(true);
    let incarnation = inject::identify(fixture.pid()).expect("identify the fixture");
    let (_directory, endpoint) = private_endpoint();

    let output =
        run_loader(fixture.pid(), incarnation.pidversion + 1, &endpoint, &fixture_executable());

    assert!(!output.status.success());
    assert!(!endpoint.exists(), "a mismatched incarnation was handed an endpoint");
    assert!(fixture.0.try_wait().expect("query fixture").is_none());
}

#[test]
#[ignore = "requires signed loader and cooperative fixtures; run via `vp run test:inject`"]
fn c_loader_refuses_an_endpoint_directory_others_can_reach() {
    let mut fixture = Fixture::launch(true);
    let incarnation = inject::identify(fixture.pid()).expect("identify the fixture");
    let (directory, endpoint) = private_endpoint();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o755))
        .expect("widen the endpoint directory");

    let output =
        run_loader(fixture.pid(), incarnation.pidversion, &endpoint, &fixture_executable());

    assert!(!output.status.success(), "a shared directory was accepted for the endpoint");
    assert!(!endpoint.exists(), "the loader bound an endpoint others could replace");
    assert!(fixture.0.try_wait().expect("query fixture").is_none());
}

fn assert_c_loader_executes(arm64e: bool) {
    let fixture = Fixture::launch(arm64e);
    let incarnation = inject::identify(fixture.pid()).expect("identify the fixture");
    let (_directory, endpoint) = private_endpoint();

    let output =
        run_loader(fixture.pid(), incarnation.pidversion, &endpoint, &fixture_executable());

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(answering_pid(&endpoint), fixture.pid());
}
