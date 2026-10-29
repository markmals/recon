#![cfg(target_os = "macos")]
//! Proposal 0005: the shipped universal C loader binds the requested incarnation and executes the
//! helper inside arm64 and arm64e targets.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use recon::macos::inject;

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
        let child = command.spawn().expect("spawn the inject fixture");
        let fixture = Self(child);
        let deadline = Instant::now() + Duration::from_secs(5);
        while inject::bind_target(fixture.pid()).is_err() {
            assert!(Instant::now() < deadline, "the fixture never became bindable");
            sleep(Duration::from_millis(20));
        }
        fixture
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

#[test]
#[ignore = "requires signed loader and cooperative fixtures; run via `vp run test:inject`"]
fn c_loader_rejects_a_mismatched_executable_before_loading() {
    let mut fixture = Fixture::launch(true);
    let incarnation = inject::bind(fixture.pid()).expect("bind the fixture").incarnation();
    let report = std::env::temp_dir().join(format!("recon-c-loader-exec-{}.pid", fixture.pid()));
    let _ = fs::remove_file(&report);

    // The incarnation is correct, but the expected executable names a different binary: exec()
    // keeps pid and pidversion, so only the executable check can catch a swapped image.
    let output = Command::new("arch")
        .arg("-arm64e")
        .arg(artifact("recon-loader"))
        .arg(fixture.pid().to_string())
        .arg(incarnation.pidversion.to_string())
        .arg(artifact("fixtures/inject-helper.dylib"))
        .arg(&report)
        .arg("/usr/bin/true")
        .output()
        .expect("run the signed C loader");
    assert!(!output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    sleep(Duration::from_millis(100));
    assert!(!report.exists(), "a mismatched executable loaded the helper");
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
    let incarnation = inject::bind(fixture.pid()).expect("bind the fixture").incarnation();
    let report = std::env::temp_dir().join(format!("recon-c-loader-stale-{}.pid", fixture.pid()));
    let _ = fs::remove_file(&report);

    let output = Command::new("arch")
        .arg("-arm64e")
        .arg(artifact("recon-loader"))
        .arg(fixture.pid().to_string())
        .arg((incarnation.pidversion + 1).to_string())
        .arg(artifact("fixtures/inject-helper.dylib"))
        .arg(&report)
        .arg(fixture_executable())
        .output()
        .expect("run the signed C loader");
    assert!(!output.status.success());
    sleep(Duration::from_millis(100));
    assert!(!report.exists(), "a mismatched incarnation loaded the helper");
    assert!(fixture.0.try_wait().expect("query fixture").is_none());
}

fn assert_c_loader_executes(arm64e: bool) {
    let fixture = Fixture::launch(arm64e);
    let incarnation = inject::bind(fixture.pid()).expect("bind the fixture").incarnation();
    let report = std::env::temp_dir().join(format!("recon-c-loader-{}.pid", fixture.pid()));
    let _ = fs::remove_file(&report);

    let output = Command::new("arch")
        .arg("-arm64e")
        .arg(artifact("recon-loader"))
        .arg(fixture.pid().to_string())
        .arg(incarnation.pidversion.to_string())
        .arg(artifact("fixtures/inject-helper.dylib"))
        .arg(&report)
        .arg(fixture_executable())
        .output()
        .expect("run the signed C loader");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let deadline = Instant::now() + Duration::from_secs(5);
    let recorded = loop {
        if let Ok(contents) = fs::read_to_string(&report) {
            break contents;
        }
        assert!(Instant::now() < deadline, "the helper never ran in the target");
        sleep(Duration::from_millis(50));
    };
    let _ = fs::remove_file(&report);
    assert_eq!(recorded.trim().parse::<i32>().expect("a pid"), fixture.pid());
}
