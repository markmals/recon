#![cfg(target_os = "macos")]
//! Proposal 0005: Recon names the incarnation the loader must bind without controlling the process
//! itself. Only the entitled loader acquires a task control port, so these hold for an unentitled
//! Recon, which is how the packaged binary ships.

use std::process::{Child, Command};

use recon_native_access_macos::current_incarnation;
use recon_native_access_macos::inject;

/// A platform binary, whose control port no unentitled process can acquire.
struct Sleeper(Child);

impl Sleeper {
    fn start() -> Self {
        Self(Command::new("/bin/sleep").arg("30").spawn().expect("start /bin/sleep"))
    }

    fn pid(&self) -> i32 {
        self.0.id() as i32
    }
}

impl Drop for Sleeper {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn identifies_a_process_it_cannot_control() {
    let sleeper = Sleeper::start();

    let incarnation = inject::identify(sleeper.pid()).expect("identify a platform binary");

    assert_eq!(incarnation.pid, sleeper.pid());
}

#[test]
fn reads_the_same_generation_as_the_process_audit_token() {
    let own = current_incarnation().expect("this process's own identity");

    let identified = inject::identify(std::process::id() as i32).expect("identify this process");

    assert_eq!((identified.pid, identified.pidversion), (own.pid, own.pidversion));
}

#[test]
fn an_exited_process_has_no_incarnation() {
    let mut sleeper = Sleeper::start();
    let pid = sleeper.pid();
    sleeper.0.kill().expect("stop the sleeper");
    sleeper.0.wait().expect("reap the sleeper");

    assert!(inject::identify(pid).is_err());
}
