#![cfg(target_os = "macos")]
//! Proposal 0005: shared WindowServer/native-window identity on a real AppKit target.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use recon::inspection::{
    AccessProfile, AcquisitionMode, CloseOutcome, Op, OpenRequest, SessionStore, Stage, close,
    native_request, open,
};
use recon::macos::inject;
use serde_json::json;
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
#[ignore = "requires the AppKit fixture, signed loader, and production helper"]
fn resolves_a_window_server_id_to_a_live_native_window() {
    let executable = artifact("fixtures/ReconFixture.app/Contents/MacOS/ReconFixture");
    let mut fixture = Fixture(Command::new(&executable).spawn().expect("launch AppKit fixture"));
    let deadline = Instant::now() + Duration::from_secs(10);
    let target = loop {
        match inject::bind_target(fixture.0.id() as i32) {
            Ok(target) => break target,
            Err(_) if Instant::now() < deadline => sleep(Duration::from_millis(25)),
            Err(error) => panic!("bind AppKit fixture: {error}"),
        }
    };
    let incarnation = target.handle.incarnation();
    let directory = TempDir::new().expect("session directory");
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
        .expect("private session directory");
    let store = SessionStore::open(directory.path()).expect("session store");
    let socket = store.endpoint_path(incarnation.into()).expect("endpoint");
    let helper_build = fs::read_to_string(artifact("recon-helper.build-id"))
        .expect("helper build id")
        .trim()
        .to_owned();
    let loaded = Command::new(artifact("recon-loader"))
        .arg(incarnation.pid.to_string())
        .arg(incarnation.pidversion.to_string())
        .arg(artifact("recon-helper.dylib"))
        .arg(&socket)
        .arg(std::fs::canonicalize(&executable).unwrap_or_else(|_| executable.clone()))
        .output()
        .expect("load helper");
    assert!(loaded.status.success(), "{}", String::from_utf8_lossy(&loaded.stderr));

    let request = OpenRequest {
        socket_path: &socket,
        bundle_path: Path::new("target/fixtures/ReconFixture.app"),
        bundle_id: "me.malstrom.recon.fixture",
        incarnation: incarnation.into(),
        mode: AcquisitionMode::Attach,
        profile: AccessProfile::Standard,
        helper_build: &helper_build,
        protocol: 1,
    };
    let opened = loop {
        match open(&store, &request) {
            Ok(opened) => break opened,
            Err(error) if error.stage() == Stage::Connect && Instant::now() < deadline => {
                sleep(Duration::from_millis(25));
            }
            Err(error) => panic!("open helper: {error}"),
        }
    };
    let session = opened.record.session_id;

    let window_id = loop {
        let windows =
            native_request(&store, &session, Op::Windows, json!({})).expect("native windows");
        let main = windows["windows"].as_array().and_then(|windows| {
            windows.iter().find(|window| {
                window["basic"]["title"]["value"].as_str() == Some("Recon Fixture — Main")
            })
        });
        if let Some(main) = main {
            break main["basic"]["windowNumber"]["value"].as_u64().expect("WindowServer id");
        }
        assert!(Instant::now() < deadline, "fixture main window was not created");
        sleep(Duration::from_millis(25));
    };

    let tree = native_request(
        &store,
        &session,
        Op::Tree,
        json!({
            "windowId": window_id,
            "kind": "view",
            "depth": 1,
            "limit": 200,
            "fields": ["basic"],
        }),
    )
    .expect("tree through the shared WindowServer id");
    assert_eq!(tree["windowRoots"].as_array().map_or(0, Vec::len), 1);
    assert!(tree["nodes"].as_array().is_some_and(|nodes| !nodes.is_empty()));

    let outcome = close(&store, &session).expect("close");
    assert!(matches!(outcome, CloseOutcome::Acknowledged { was_active: true }));
    assert!(fixture.0.try_wait().expect("target liveness").is_none());
}
