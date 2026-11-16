#![cfg(target_os = "macos")]

use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread::{self, sleep};
use std::time::{Duration, Instant};

use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_core_graphics::{
    CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID, kCGWindowOwnerPID,
};
use recon_native_access_macos::identity::{AccessMode, AccessProfile, Incarnation};
use recon_native_access_macos::inject;
use recon_native_access_macos::{
    HELPER_PROTOCOL, OpenRequest, SessionState, SessionStore, Stage, open,
};
use serde_json::{Value, json};

fn artifact(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target").join(name)
}

struct OwnedFixture(Child);

impl Drop for OwnedFixture {
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

#[test]
#[ignore = "requires vp run test:inspection artifacts and a signed cooperative inject fixture"]
fn an_unconfirmed_close_is_a_typed_cli_failure_not_success_json() {
    let executable = artifact("fixtures/inject-fixture");
    let mut fixture = OwnedFixture(
        Command::new("arch")
            .arg("-arm64e")
            .arg(&executable)
            .stdout(Stdio::piped())
            .spawn()
            .expect("start a controlled cooperative target"),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    wait_for_fixture(&mut fixture.0);
    let incarnation = inject::identify(fixture.0.id() as i32).expect("identify the target");

    let home = tempfile::Builder::new().prefix("rcn").tempdir_in("/tmp").expect("private HOME");
    let root = home.path().join("Library/Application Support/recon/inspection");
    let store = SessionStore::open(&root).expect("private session store");
    let socket = store.endpoint_path(incarnation.into()).expect("helper endpoint path");
    let loaded = Command::new(artifact("recon-loader"))
        .arg(incarnation.pid.to_string())
        .arg(incarnation.pidversion.to_string())
        .arg(artifact("recon-helper.dylib"))
        .arg(&socket)
        .arg(executable.canonicalize().expect("canonical executable"))
        .output()
        .expect("start the bound loader");
    assert!(loaded.status.success(), "{}", String::from_utf8_lossy(&loaded.stderr));
    let helper_build = fs::read_to_string(artifact("recon-helper.build-id"))
        .expect("matching helper build")
        .trim()
        .to_owned();
    let request = OpenRequest {
        socket_path: &socket,
        bundle_path: Path::new("/tmp/recon-fixture.app"),
        bundle_id: "me.malstrom.recon.fixture",
        incarnation: incarnation.into(),
        mode: AccessMode::Attach,
        profile: AccessProfile::Standard,
        helper_build: &helper_build,
        protocol: HELPER_PROTOCOL,
    };
    let session = loop {
        match open(&store, &request) {
            Ok(opened) => break opened.record.session_id,
            Err(error) if error.stage() == Stage::Connect && Instant::now() < deadline => {
                sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("open controlled helper: {error}"),
        }
    };

    fs::set_permissions(&socket, fs::Permissions::from_mode(0o000))
        .expect("simulate an unreachable bootstrap endpoint");
    let output = Command::new(env!("CARGO_BIN_EXE_recon"))
        .args(["ui", "session", "close", session.as_str()])
        .env("HOME", home.path())
        .output()
        .expect("request close through the CLI");

    assert!(!output.status.success(), "an unacknowledged close is not success");
    assert!(output.stdout.is_empty(), "failed close must not print success JSON");
    let failure: Value = serde_json::from_slice(&output.stderr).expect("one typed error on stderr");
    assert_eq!(failure["error"]["details"]["session"], session.as_str());
    assert_eq!(store.record(&session).unwrap().unwrap().state, SessionState::CloseUnconfirmed);
    assert!(fixture.0.try_wait().expect("target still running").is_none());
}

#[test]
#[ignore = "requires macOS 27 Apple silicon, signed cooperative ReconFixture.app, signed recon-loader, and built recon-helper.dylib/build-id"]
fn proposal_0005_pending_attach_without_endpoint_refuses_reinjection() {
    let bundle = artifact("fixtures/ReconFixture.app");
    let executable = bundle.join("Contents/MacOS/ReconFixture");
    let mut fixture =
        OwnedFixture(Command::new(&executable).spawn().expect("launch own AppKit fixture"));
    let deadline = Instant::now() + Duration::from_secs(10);
    let incarnation: Incarnation =
        inject::identify(fixture.0.id() as i32).expect("identify AppKit fixture").into();
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

/// How many windows the window server lists for the fixture process.
fn fixture_window_count(pid: i32) -> usize {
    let Some(listed) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, kCGNullWindowID)
    else {
        return 0;
    };
    // SAFETY: the window server describes each window with a dictionary keyed by CFString.
    let listed: CFRetained<CFArray<CFDictionary<CFString, CFType>>> =
        unsafe { CFRetained::cast_unchecked(listed) };
    listed
        .iter()
        .filter(|entry| {
            // SAFETY: `kCGWindowOwnerPID` names a CFNumber in every window description.
            let owner = unsafe { entry.get(kCGWindowOwnerPID) };
            owner
                .and_then(|owner| owner.downcast_ref::<CFNumber>().and_then(CFNumber::as_i32))
                .is_some_and(|owner| owner == pid)
        })
        .count()
}
