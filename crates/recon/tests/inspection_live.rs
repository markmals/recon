#![cfg(target_os = "macos")]
//! Proposal 0005: a real loader/helper handshake and acknowledged close through the host transport.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_core_graphics::{
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowOwnerPID,
};
use recon::inspection::{
    AccessProfile, AcquisitionMode, CloseOutcome, Incarnation, OpenRequest, SessionStore, Stage,
    close, open,
};
use recon::macos::inject;
use serde_json::{Value, json};
use tempfile::TempDir;

fn artifact(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target").join(name)
}

struct Fixture(Child);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
#[ignore = "requires signed loader, production helper, and cooperative fixture"]
fn opens_and_closes_a_real_helper_session() {
    let binary = artifact("fixtures/inject-fixture");
    let mut fixture = Fixture(
        Command::new("arch")
            .arg("-arm64e")
            .arg(binary)
            .spawn()
            .expect("launch cooperative fixture"),
    );
    let bind_deadline = Instant::now() + Duration::from_secs(5);
    let target = loop {
        match inject::bind_target(fixture.0.id() as i32) {
            Ok(target) => break target,
            Err(_) if Instant::now() < bind_deadline => sleep(Duration::from_millis(20)),
            Err(error) => panic!("bind target: {error}"),
        }
    };
    let incarnation = target.handle.incarnation();

    let directory = TempDir::new().expect("session directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private session directory");
    let store = SessionStore::open(directory.path()).expect("open session store");
    let socket = store.endpoint_path(incarnation.into()).expect("endpoint path");
    let helper = artifact("recon-helper.dylib");
    let helper_build = fs::read_to_string(artifact("recon-helper.build-id"))
        .expect("helper build id")
        .trim()
        .to_owned();

    let output = Command::new("arch")
        .arg("-arm64e")
        .arg(artifact("recon-loader"))
        .arg(incarnation.pid.to_string())
        .arg(incarnation.pidversion.to_string())
        .arg(helper)
        .arg(&socket)
        .arg(
            std::fs::canonicalize(artifact("fixtures/inject-fixture"))
                .unwrap_or(artifact("fixtures/inject-fixture")),
        )
        .output()
        .expect("run loader");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));

    let request = OpenRequest {
        socket_path: &socket,
        bundle_path: Path::new("/tmp/recon-fixture.app"),
        bundle_id: "me.malstrom.recon.fixture",
        incarnation: incarnation.into(),
        mode: AcquisitionMode::Attach,
        profile: AccessProfile::Standard,
        helper_build: &helper_build,
        protocol: 1,
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    let opened = loop {
        match open(&store, &request) {
            Ok(opened) => break opened,
            Err(error) if error.stage() == Stage::Connect => {
                assert!(Instant::now() < deadline, "helper did not start: {error}");
                sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("helper handshake failed: {error}"),
        }
    };
    assert!(!opened.reused);
    assert!(opened.response.get("capability").is_none());
    assert!(store.record(&opened.record.session_id).unwrap().is_some());

    let outcome = close(&store, &opened.record.session_id).expect("close session");
    assert!(matches!(outcome, CloseOutcome::Acknowledged { was_active: true }));
    assert!(fixture.0.try_wait().expect("target liveness").is_none());
}

fn fixture_window_count(pid: i32) -> usize {
    let Some(windows) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, kCGNullWindowID)
    else {
        return 0;
    };
    // SAFETY: WindowServer supplies a CFArray of dictionaries keyed by CFString.
    let windows: CFRetained<CFArray<CFDictionary<CFString, CFType>>> =
        unsafe { CFRetained::cast_unchecked(windows) };
    // SAFETY: kCGWindowOwnerPID is a documented immutable WindowServer dictionary key.
    let owner_key = unsafe { kCGWindowOwnerPID };
    windows
        .iter()
        .filter(|window| {
            window
                .get(owner_key)
                .and_then(|owner| owner.downcast::<CFNumber>().ok())
                .and_then(|owner| owner.as_i32())
                == Some(pid)
        })
        .count()
}

#[test]
#[ignore = "requires macOS 27 Apple silicon, signed cooperative ReconFixture.app, signed recon-loader, and built recon-helper.dylib/build-id"]
fn proposal_0005_pending_attach_without_endpoint_refuses_reinjection() {
    let bundle = artifact("fixtures/ReconFixture.app");
    let executable = bundle.join("Contents/MacOS/ReconFixture");
    let mut fixture =
        Fixture(Command::new(&executable).spawn().expect("launch own AppKit fixture"));
    let deadline = Instant::now() + Duration::from_secs(10);
    let target = loop {
        match inject::bind_target(fixture.0.id() as i32) {
            Ok(target) => break target,
            Err(_) if Instant::now() < deadline => sleep(Duration::from_millis(25)),
            Err(error) => panic!("bind AppKit fixture: {error}"),
        }
    };
    let incarnation: Incarnation = target.handle.incarnation().into();
    while fixture_window_count(incarnation.pid) < 2 && Instant::now() < deadline {
        sleep(Duration::from_millis(25));
    }
    assert!(fixture_window_count(incarnation.pid) >= 2, "fixture windows did not appear");
    // The no-focus guard watches for transient window churn; begin only after startup settles.
    sleep(Duration::from_millis(300));
    let home = tempfile::Builder::new().prefix("rcn").tempdir_in("/tmp").expect("private HOME");
    let root = home.path().join("Library/Application Support/recon/inspection");
    let store = SessionStore::open(&root).expect("open the isolated session store");
    let socket = store.endpoint_path(incarnation).expect("endpoint path");
    assert!(!socket.exists(), "the fixture starts without a helper");

    // Model a previous CLI that exited after claiming this exact executable, before its handshake.
    let reservations = root.join("attaches");
    if !reservations.exists() {
        fs::create_dir(&reservations).expect("reservation directory");
    }
    fs::set_permissions(&reservations, fs::Permissions::from_mode(0o700))
        .expect("owner-private reservation directory");
    let reservation = reservations.join(format!("{}.json", incarnation.slug()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&reservation)
        .expect("owner-private attach reservation");
    file.write_all(
        &serde_json::to_vec(&json!({
            "version": 1,
            "pid": incarnation.pid,
            "pidversion": incarnation.pidversion,
            "executablePath": executable.canonicalize().expect("canonical fixture executable"),
        }))
        .expect("encode the reservation"),
    )
    .expect("write the reservation");
    file.sync_all().expect("persist the attach reservation");
    drop(file);

    let response = Command::new(env!("CARGO_BIN_EXE_recon"))
        .args(["ui", "session", "open"])
        .arg(&bundle)
        .args(["--mode", "attach", "--pid"])
        .arg(incarnation.pid.to_string())
        .env("HOME", home.path())
        .output()
        .expect("open the exact fixture through the CLI");
    let error: Value = serde_json::from_slice(&response.stderr).unwrap_or_else(|_| {
        panic!(
            "expected one typed CLI failure; stderr: {}",
            String::from_utf8_lossy(&response.stderr)
        )
    });
    assert_eq!(error["error"]["code"], "INSPECTION_BUSY", "{error}");
    assert_eq!(error["error"]["details"]["targetExecutionMayContinue"], true, "{error}");
    assert!(!socket.exists(), "uncertain attach must not load a second helper");
    assert!(reservation.exists(), "unknown attach outcome remains durable");
    assert!(store.records().expect("read the session store").is_empty());
    assert!(fixture.0.try_wait().expect("fixture liveness").is_none());
}
