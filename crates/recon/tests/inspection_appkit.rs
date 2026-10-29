#![cfg(target_os = "macos")]
//! Proposal 0005: shared WindowServer/native-window identity on a real AppKit target.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use objc2_core_foundation::{
    CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType, CGRect,
};
use objc2_core_graphics::{
    CGRectMakeWithDictionaryRepresentation, CGWindowListCopyWindowInfo, CGWindowListOption,
    kCGNullWindowID, kCGWindowBounds, kCGWindowNumber, kCGWindowOwnerPID,
};
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

fn fixture_main_window_server_id(pid: u32) -> Option<u64> {
    let listed = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, kCGNullWindowID)?;
    // SAFETY: WindowServer returns dictionaries keyed by CFString.
    let listed: CFRetained<CFArray<CFDictionary<CFString, CFType>>> =
        unsafe { CFRetained::cast_unchecked(listed) };
    // SAFETY: these are documented immutable WindowServer dictionary keys.
    let (owner_key, bounds_key, number_key) =
        unsafe { (kCGWindowOwnerPID, kCGWindowBounds, kCGWindowNumber) };
    listed.iter().find_map(|entry| {
        let owner = entry.get(owner_key)?.downcast::<CFNumber>().ok()?;
        if owner.as_i64()? != i64::from(pid) {
            return None;
        }
        let bounds = entry.get(bounds_key)?.downcast::<CFDictionary>().ok()?;
        let mut frame = CGRect::ZERO;
        // SAFETY: the dictionary is a WindowServer bounds value; frame is a writable CGRect.
        if !unsafe { CGRectMakeWithDictionaryRepresentation(Some(&bounds), &raw mut frame) } {
            return None;
        }
        // The controlled main window is at x=160 and is 480x352 including its title bar.
        // Other target-owned windows can be wider than 400 points; width alone is not an oracle.
        if frame.origin.x != 160.0 || frame.size.width != 480.0 || frame.size.height != 352.0 {
            return None;
        }
        let id = entry.get(number_key)?.downcast::<CFNumber>().ok()?;
        u64::try_from(id.as_i64()?).ok()
    })
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
            if let Some(independent) = fixture_main_window_server_id(fixture.0.id())
                && let Some(mapped) = main["windowId"].as_u64()
            {
                assert_eq!(
                    mapped, independent,
                    "native and external inventories must name one window"
                );
                break mapped;
            }
            assert!(
                Instant::now() < deadline,
                "fixture main window never gained a verified WindowServer mapping: {main}"
            );
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
