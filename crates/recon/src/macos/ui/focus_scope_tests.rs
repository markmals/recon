use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use super::scope_of;
use crate::application::Pid;
use crate::inspection::{
    AccessProfile as SessionProfile, AcquisitionMode, Capability, Incarnation, SessionId,
    SessionRecord, SessionState, SessionStore,
};
use crate::ui::request::ViewRequest;
use crate::ui::request::{AccessMode, AccessProfile, Selection, SessionRequest, UiRequest};

struct OwnedFixture(Pid);

impl Drop for OwnedFixture {
    fn drop(&mut self) {
        let _ = Command::new("/bin/kill").arg(self.0.0.to_string()).status();
    }
}

fn running_pid(executable: &Path) -> Option<Pid> {
    let output = Command::new("/usr/bin/pgrep")
        .arg("-f")
        .arg(executable)
        .output()
        .expect("query the fixture process");
    let pids: Vec<Pid> = String::from_utf8(output.stdout)
        .expect("numeric process identifiers")
        .lines()
        .map(|line| Pid(line.parse().expect("numeric PID")))
        .collect();
    assert!(pids.len() < 2, "a different fixture is running: {pids:?}");
    pids.first().copied()
}

#[test]
#[ignore = "requires the built AppKit fixture; run explicitly with the desktop hands-off"]
fn an_implicit_attach_pid_is_the_instance_the_guard_watches() {
    let bundle = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fixtures/ReconFixture.app")
        .canonicalize()
        .expect("built AppKit fixture");
    let executable = bundle.join("Contents/MacOS/ReconFixture");
    assert!(running_pid(&executable).is_none(), "a fixture is already running");
    let launched = Command::new("/usr/bin/open")
        .arg("-g")
        .arg("-n")
        .arg(&bundle)
        .status()
        .expect("launch a private fixture in the background");
    assert!(launched.success());

    let deadline = Instant::now() + Duration::from_secs(5);
    let pid = loop {
        if let Some(pid) = running_pid(&executable) {
            break pid;
        }
        assert!(Instant::now() < deadline, "fixture did not start");
        sleep(Duration::from_millis(20));
    };
    let _fixture = OwnedFixture(pid);
    let request = UiRequest::Session(SessionRequest::Open {
        selection: Selection { target: bundle.to_string_lossy().into_owned(), pid: None },
        mode: AccessMode::Attach,
        profile: AccessProfile::Standard,
        allow_authorization: true,
    });
    let watched = loop {
        match scope_of(&request) {
            Ok(scope) => break scope.target,
            Err(error) if Instant::now() < deadline => {
                let _ = error;
                sleep(Duration::from_millis(20));
            }
            Err(error) => panic!("fixture was not selectable: {error:?}"),
        }
    };
    assert_eq!(watched, Some(pid), "the prompt exception must not hide target activation");
}

#[test]
#[ignore = "requires built AppKit fixture and a hands-off macOS desktop"]
fn proposal0005_session_bound_guard_watches_its_bound_pid() {
    if std::env::var_os("RECON_SCOPE_TEST_CHILD").is_none() {
        let home = tempfile::Builder::new().prefix("rcn").tempdir_in("/tmp").unwrap();
        let result = Command::new(std::env::current_exe().expect("test executable"))
            .args([
                "--exact",
                "macos::ui::focus_scope_tests::proposal0005_session_bound_guard_watches_its_bound_pid",
                "--ignored",
            ])
            .env("RECON_SCOPE_TEST_CHILD", "1")
            .env("HOME", home.path())
            .output()
            .expect("run guard test in isolated session store");
        assert!(
            result.status.success(),
            "stdout: {}; stderr: {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            String::from_utf8_lossy(&result.stdout).contains("1 passed"),
            "guard test did not execute: {}",
            String::from_utf8_lossy(&result.stdout)
        );
        return;
    }

    let bundle = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/fixtures/ReconFixture.app")
        .canonicalize()
        .expect("built AppKit fixture");
    let executable = bundle.join("Contents/MacOS/ReconFixture");
    assert!(running_pid(&executable).is_none(), "a fixture is already running");
    let mut owned = Vec::new();
    for count in 1..=2 {
        assert!(
            Command::new("/usr/bin/open")
                .args(["-g", "-n"])
                .arg(&bundle)
                .status()
                .expect("launch background fixture")
                .success()
        );
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let output = Command::new("/usr/bin/pgrep")
                .arg("-f")
                .arg(&executable)
                .output()
                .expect("find fixtures");
            let pids = String::from_utf8(output.stdout)
                .expect("numeric process identifiers")
                .lines()
                .map(|line| Pid(line.parse().expect("numeric PID")))
                .collect::<Vec<_>>();
            for pid in &pids {
                if !owned.iter().any(|own: &OwnedFixture| own.0 == *pid) {
                    owned.push(OwnedFixture(*pid));
                }
            }
            if pids.len() == count {
                break;
            }
            assert!(Instant::now() < deadline, "expected {count} fixture instances, got {pids:?}");
            sleep(Duration::from_millis(25));
        }
    }

    let bound = owned[0].0;
    let other = owned[1].0;
    let store = SessionStore::open(SessionStore::default_root().unwrap()).unwrap();
    let id = SessionId::generate().unwrap();
    let incarnation = Incarnation::new(bound.0 as i32, 1);
    store
        .insert(&SessionRecord {
            session_id: id.clone(),
            capability: Capability::generate().unwrap(),
            bundle_path: bundle.clone(),
            bundle_id: "me.malstrom.recon.fixture".to_owned(),
            incarnation,
            established_mode: AcquisitionMode::Attach,
            requested_mode: AcquisitionMode::Attach,
            profile: SessionProfile::Standard,
            helper_build: "test-only-local-binding".to_owned(),
            protocol: 1,
            socket_path: store.endpoint_path(incarnation).unwrap(),
            state: SessionState::Active,
            opened_at: 0,
            updated_at: 0,
        })
        .unwrap();
    let selection = Selection { target: bundle.to_string_lossy().into_owned(), pid: None };
    let request = UiRequest::Windows { selection: selection.clone(), session: Some(id.clone()) };
    assert_eq!(scope_of(&request).expect("session selects an instance").target, Some(bound));

    let wrong = Selection { pid: Some(other), ..selection };
    for request in [
        UiRequest::Windows { selection: wrong.clone(), session: Some(id.clone()) },
        UiRequest::View(ViewRequest::Tree {
            selection: wrong,
            session: id,
            scope: Default::default(),
            kind: crate::ui::request::TraversalKind::View,
            depth: 0,
            fields: Default::default(),
            limit: 1,
        }),
    ] {
        match scope_of(&request) {
            Err(error) => assert_eq!(error.code, crate::error::ErrorCode::StaleSession),
            Ok(_) => panic!("a different PID cannot borrow the session"),
        }
    }
}
