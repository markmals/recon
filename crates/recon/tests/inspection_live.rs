#![cfg(target_os = "macos")]
//! Proposal 0005: a real loader/helper handshake and acknowledged close through the host transport.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use recon::inspection::{
    AccessProfile, AcquisitionMode, CloseOutcome, OpenRequest, SessionStore, Stage, close, open,
};
use recon::macos::inject;
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
