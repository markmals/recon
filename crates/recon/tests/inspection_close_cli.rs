#![cfg(target_os = "macos")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use recon::inspection::{
    AccessProfile, AcquisitionMode, OpenRequest, SessionState, SessionStore, Stage, open,
};
use recon::macos::inject;
use serde_json::Value;

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

#[test]
#[ignore = "requires vp run test:inspection artifacts and a signed cooperative inject fixture"]
fn an_unconfirmed_close_is_a_typed_cli_failure_not_success_json() {
    let executable = artifact("fixtures/inject-fixture");
    let mut fixture = OwnedFixture(
        Command::new("arch")
            .arg("-arm64e")
            .arg(&executable)
            .spawn()
            .expect("start a controlled cooperative target"),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    let incarnation = loop {
        match inject::bind_target(fixture.0.id() as i32) {
            Ok(target) => break target.handle.incarnation(),
            Err(_) if Instant::now() < deadline => sleep(Duration::from_millis(20)),
            Err(error) => panic!("bind the target: {error}"),
        }
    };

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
        mode: AcquisitionMode::Attach,
        profile: AccessProfile::Standard,
        helper_build: &helper_build,
        protocol: 1,
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
