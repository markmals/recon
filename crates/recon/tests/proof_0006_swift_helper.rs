#![cfg(target_os = "macos")]
//! Throwaway feasibility proof for proposal.0006, not a permanent test: a helper whose `windows`
//! read is a generated Swift action, loaded by the existing C loader into arm64 and arm64e AppKit
//! targets and driven through the existing host transport. Build it with `node proof/0006/build.ts`.

use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::{self, sleep};
use std::time::{Duration, Instant};

use recon::inspection::{
    AccessProfile, AcquisitionMode, CloseOutcome, InspectionErrorCode, Op, OpenRequest, SessionId,
    SessionStore, Stage, close, current_incarnation, native_request, open,
};
use recon::macos::inject::{self, Incarnation};
use serde_json::{Map, Value, json};
use tempfile::TempDir;

fn target(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target").join(name)
}

struct Fixture(Child);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

struct Attached {
    fixture: Fixture,
    directory: TempDir,
    incarnation: Incarnation,
    socket: PathBuf,
    helper_build: String,
}

impl Attached {
    fn launch(arch: &str, executable: PathBuf) -> Self {
        let fixture = Fixture(
            Command::new("arch")
                .arg(format!("-{arch}"))
                .arg(&executable)
                .spawn()
                .expect("launch fixture"),
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        let bound = loop {
            match inject::bind_target(fixture.0.id() as i32) {
                Ok(bound) => break bound,
                Err(_) if Instant::now() < deadline => sleep(Duration::from_millis(25)),
                Err(error) => panic!("bind proof fixture: {error}"),
            }
        };
        let incarnation = bound.handle.incarnation();
        let directory = TempDir::new().expect("session directory");
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
        let store = SessionStore::open(directory.path()).expect("session store");
        let socket = store.endpoint_path(incarnation.into()).expect("endpoint");
        let helper_build = fs::read_to_string(target("proof-0006/recon-helper.build-id"))
            .expect("helper build id")
            .trim()
            .to_owned();
        let loaded = Command::new(target("recon-loader"))
            .arg(incarnation.pid.to_string())
            .arg(incarnation.pidversion.to_string())
            .arg(target("proof-0006/recon-helper.dylib"))
            .arg(&socket)
            .arg(fs::canonicalize(&executable).unwrap())
            .output()
            .expect("run loader");
        assert!(loaded.status.success(), "{}", String::from_utf8_lossy(&loaded.stderr));
        Self { fixture, directory, incarnation, socket, helper_build }
    }

    fn store(&self) -> SessionStore {
        SessionStore::open(self.directory.path()).expect("session store")
    }

    fn open(&self) -> (SessionId, Map<String, Value>) {
        let request = OpenRequest {
            socket_path: &self.socket,
            bundle_path: Path::new("target/proof-0006/proof-fixture"),
            bundle_id: "proof-fixture",
            incarnation: self.incarnation.into(),
            mode: AcquisitionMode::Attach,
            profile: AccessProfile::Standard,
            helper_build: &self.helper_build,
            protocol: 1,
        };
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match open(&self.store(), &request) {
                Ok(opened) => break (opened.record.session_id, opened.response),
                Err(error) if error.stage() == Stage::Connect && Instant::now() < deadline => {
                    sleep(Duration::from_millis(25));
                }
                Err(error) => panic!("open helper session: {error}"),
            }
        }
    }

    fn windows(&self, session: &SessionId, params: Value) -> Map<String, Value> {
        native_request(&self.store(), session, Op::Windows, params).expect("windows read")
    }

    /// One `windows` frame straight to the helper, waiting longer than the host's own deadline.
    fn raw_windows(&self, session: &SessionId) -> Value {
        let record = self.store().record(session).unwrap().expect("session record");
        let client = current_incarnation().expect("own incarnation");
        let body = json!({
            "op": "windows", "protocol": 1, "helperBuild": self.helper_build,
            "clientPid": client.pid, "clientPidversion": client.pidversion,
            // SAFETY: getuid has no preconditions.
            "clientUid": unsafe { libc::getuid() },
            "sessionId": record.session_id.as_str(), "capability": record.capability.expose(),
        });
        let mut stream = UnixStream::connect(&self.socket).expect("connect helper");
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let payload = serde_json::to_vec(&body).unwrap();
        stream.write_all(&(payload.len() as u32).to_be_bytes()).unwrap();
        stream.write_all(&payload).unwrap();
        let mut length = [0u8; 4];
        stream.read_exact(&mut length).unwrap();
        let mut response = vec![0u8; u32::from_be_bytes(length) as usize];
        stream.read_exact(&mut response).unwrap();
        serde_json::from_slice(&response).unwrap()
    }

    fn signal(&self, name: &str) {
        let status = Command::new("kill")
            .arg(format!("-{name}"))
            .arg(self.incarnation.pid.to_string())
            .status()
            .expect("signal fixture");
        assert!(status.success());
    }

    fn alive(&mut self) -> bool {
        self.fixture.0.try_wait().expect("fixture liveness").is_none()
    }
}

fn invocations(observation: &Map<String, Value>) -> u64 {
    observation["invocations"].as_u64().expect("invocation count")
}

fn prove(arch: &str) {
    let mut attached = Attached::launch(arch, target("proof-0006/proof-fixture"));
    let (session, opened) = attached.open();
    assert_eq!(opened["identity"]["architecture"], arch, "{opened:?}");

    let observed = attached.windows(&session, json!({}));
    println!("{arch} windows: {}", Value::Object(observed.clone()));
    assert_eq!(observed["mainThread"], true);
    let main = observed["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|window| window["title"]["value"] == "Proof — Main")
        .expect("fixture window");
    let view = |identifier: &str| {
        main["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|view| view["identifier"]["value"] == identifier)
            .cloned()
            .unwrap()
    };
    assert_eq!(view("plain")["toolTip"], json!({"status": "value", "value": "plain tip"}));
    assert_eq!(view("raising")["toolTip"], json!({"status": "failed"}));
    let baseline = invocations(&observed);

    // Unknown members are rejected against the generated schema before the body runs.
    let rejected = native_request(&attached.store(), &session, Op::Windows, json!({"depth": 2}))
        .expect_err("unknown member");
    println!("{arch} unknown member: {:?} {}", rejected.code(), rejected.message());
    assert_eq!(rejected.code(), InspectionErrorCode::InvalidArgument);
    assert_eq!(invocations(&attached.windows(&session, json!({}))), baseline + 1);

    // A read queued behind a blocked main thread past its deadline never starts. The host transport
    // shares the helper's 2 s budget and so conservatively reports that execution may continue
    // before the helper answers; ask the helper directly to observe its own classification.
    attached.signal("USR1");
    sleep(Duration::from_millis(100));
    let expired = attached.raw_windows(&session);
    println!("{arch} queued expiry (helper): {expired}");
    assert_eq!(expired["error"]["code"], "INSPECTION_TIMEOUT");
    assert_eq!(expired["error"]["stage"], "main-thread-queue");
    assert_eq!(expired["error"]["targetExecutionMayContinue"], false);
    sleep(Duration::from_millis(1200));
    assert_eq!(invocations(&attached.windows(&session, json!({}))), baseline + 2);

    // Closing while a read is queued revokes it before it starts.
    attached.signal("USR2");
    sleep(Duration::from_millis(100));
    let directory = attached.directory.path().to_owned();
    let queued_session = session.clone();
    let queued = thread::spawn(move || {
        let store = SessionStore::open(directory).expect("session store");
        native_request(&store, &queued_session, Op::Windows, json!({}))
    });
    sleep(Duration::from_millis(200));
    let closed = close(&attached.store(), &session).expect("close while queued");
    assert!(matches!(closed, CloseOutcome::Acknowledged { was_active: true }), "{closed:?}");
    let revoked = queued.join().unwrap().expect_err("queued read after close");
    println!("{arch} queued close: {:?} {}", revoked.code(), revoked.message());
    assert_eq!(revoked.code(), InspectionErrorCode::StaleSession);
    sleep(Duration::from_millis(1000));
    let (reopened, _) = attached.open();
    assert_eq!(invocations(&attached.windows(&reopened, json!({}))), baseline + 3);

    // Ownership across the generated adapter: repeated reads, then a leak scan of the target.
    for _ in 0..200 {
        attached.windows(&reopened, json!({"limit": 25}));
    }
    let leaks = Command::new("leaks").arg(attached.incarnation.pid.to_string()).output().unwrap();
    let report = String::from_utf8_lossy(&leaks.stdout);
    let summary = report.lines().find(|line| line.contains("leaks for")).unwrap_or("no summary");
    println!("{arch} leaks: {summary}");
    assert!(summary.contains(" 0 leaks for "), "{report}");

    assert!(matches!(
        close(&attached.store(), &reopened).expect("close"),
        CloseOutcome::Acknowledged { was_active: true }
    ));
    assert!(attached.alive(), "the target survived every read, including the raising getter");
}

#[test]
#[ignore = "proposal.0006 proof: needs node proof/0006/build.ts and mise run loader:build"]
fn swift_action_helper_inspects_an_arm64_appkit_target() {
    prove("arm64");
}

#[test]
#[ignore = "proposal.0006 proof: needs node proof/0006/build.ts and mise run loader:build"]
fn swift_action_helper_inspects_an_arm64e_appkit_target() {
    prove("arm64e");
}

/// A Swift-language AppKit target already has the OS Swift runtime loaded before injection.
#[test]
#[ignore = "proposal.0006 proof: also needs mise run fixture:build"]
fn swift_action_helper_inspects_a_swift_appkit_target() {
    let mut attached =
        Attached::launch("arm64", target("fixtures/ReconFixture.app/Contents/MacOS/ReconFixture"));
    let (session, _) = attached.open();
    let deadline = Instant::now() + Duration::from_secs(10);
    let observed = loop {
        let observed = attached.windows(&session, json!({}));
        let titled = observed["windows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|window| window["title"]["value"] == "Recon Fixture — Main");
        if titled || Instant::now() >= deadline {
            break observed;
        }
        sleep(Duration::from_millis(50));
    };
    println!("swift target windows: {}", Value::Object(observed.clone()));
    assert_eq!(observed["mainThread"], true);
    assert!(
        observed["windows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|window| { window["title"]["value"] == "Recon Fixture — Main" })
    );
    assert!(matches!(
        close(&attached.store(), &session).expect("close"),
        CloseOutcome::Acknowledged { was_active: true }
    ));
    assert!(attached.alive());
}
