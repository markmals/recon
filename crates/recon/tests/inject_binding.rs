#![cfg(target_os = "macos")]
//! Proposal 0005: the native loader binds to one kernel-verified process incarnation and confirms
//! the target shares this process's dyld shared cache before any effect. These run against a
//! cooperative `get-task-allow` fixture; they acquire a task control port and read only.

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::thread::sleep;
use std::time::{Duration, Instant};

use recon::macos::inject::{self, Incarnation, LoaderError};

fn fixture_binary() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/fixtures/inject-fixture")
}

/// A running cooperative fixture, killed and reaped when dropped.
struct Fixture(Child);

impl Fixture {
    fn launch() -> Self {
        let binary = fixture_binary();
        assert!(
            binary.is_file(),
            "{} is missing; build it with `vp run fixture:inject`",
            binary.display()
        );
        let child = Command::new(&binary).spawn().expect("spawn the inject fixture");
        let fixture = Self(child);
        // Wait until the process has started far enough to be bindable.
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
#[ignore = "requires the cooperative inject fixture; run via `vp run test:inject`"]
fn binds_to_the_running_incarnation_sharing_this_process_cache() {
    let fixture = Fixture::launch();
    let target = inject::bind_target(fixture.pid()).expect("bind the cooperative fixture");

    assert_eq!(target.handle.incarnation().pid, fixture.pid());
    // A shared cache was found and matched, so the loader could resolve libSystem addresses.
    assert_ne!(target.shared_cache.base, 0);
    // Revalidating against the same live incarnation succeeds.
    target.handle.revalidate().expect("still the same incarnation");
}

#[test]
#[ignore = "requires the cooperative inject fixture; run via `vp run test:inject`"]
fn a_reused_pid_generation_is_refused_without_touching_the_target() {
    let fixture = Fixture::launch();
    let live = inject::bind_target(fixture.pid()).expect("bind").handle.incarnation();
    let impostor = Incarnation { pid: live.pid, pidversion: live.pidversion.wrapping_add(1) };

    match inject::rebind_target(fixture.pid(), impostor) {
        Err(LoaderError::IncarnationMismatch { expected, found }) => {
            assert_eq!(expected, impostor);
            assert_eq!(found, live);
        }
        other => panic!("expected an incarnation mismatch, got {other:?}"),
    }
}

#[test]
#[ignore = "requires the cooperative inject fixture; run via `vp run test:inject`"]
fn an_exited_target_is_no_longer_bindable() {
    let fixture = Fixture::launch();
    let pid = fixture.pid();
    let target = inject::bind_target(pid).expect("bind the live fixture");

    drop(fixture); // kills and reaps the process

    // The recorded port now names a dead task, and a fresh bind of the dead pid also refuses,
    // rather than inventing or inspecting a replacement.
    assert!(target.handle.revalidate().is_err());
    assert!(inject::bind_target(pid).is_err());
}
