//! What a target's own sandbox lets it do with the helper Recon would load into it.
//!
//! The loader's bootstrap opens the helper, and the helper binds its endpoint, from inside the
//! target and under the target's sandbox. When that sandbox denies either, the loader still
//! succeeds: the helper never loads or never listens, and the open can only time out with the
//! bootstrap already in the target. Asking the target's sandbox first turns that into a refusal
//! before any effect.

use std::ffi::{CStr, CString, c_char};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// `SANDBOX_FILTER_PATH`: the operation's one argument is a C-string path.
const FILTER_PATH: libc::c_int = 1;

unsafe extern "C" {
    /// Queries another process's sandbox: 0 when it allows the operation, 1 when it denies it, and
    /// -1 when the query itself failed.
    fn sandbox_check(
        pid: libc::pid_t,
        operation: *const c_char,
        filter: libc::c_int,
        ...
    ) -> libc::c_int;
    /// Flag for `sandbox_check` that asks without recording a violation.
    static SANDBOX_CHECK_NO_REPORT: libc::c_int;
}

/// Why the sandbox of the process running as `pid` would stop the helper at `helper` from loading,
/// or from listening at `endpoint`; `None` when it allows both or cannot say. The sandbox answers
/// only for paths that exist, so the endpoint, which the helper creates, is judged by its
/// directory.
pub(crate) fn load_refusal(pid: i32, helper: &Path, endpoint: &Path) -> Option<String> {
    let mut refusals = Vec::new();
    if denies(pid, c"file-read-data", helper) || denies(pid, c"file-map-executable", helper) {
        refusals.push(format!(
            "it cannot read the helper at {}; a sandboxed target loads the helper only from a \
             location its sandbox reads, such as /Library/Application Support/Recon",
            helper.display()
        ));
    }
    if let Some(directory) = endpoint.parent()
        && denies(pid, c"network-bind", directory)
    {
        refusals.push(format!("it cannot create the helper endpoint in {}", directory.display()));
    }
    (!refusals.is_empty()).then(|| {
        format!(
            "the target's sandbox would stop the helper, so nothing was loaded: {}",
            refusals.join("; ")
        )
    })
}

fn denies(pid: i32, operation: &CStr, path: &Path) -> bool {
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: both strings are NUL-terminated and outlive the call, and the path filter takes
    // exactly one C-string argument.
    unsafe {
        let filter = FILTER_PATH | SANDBOX_CHECK_NO_REPORT;
        sandbox_check(pid, operation.as_ptr(), filter, path.as_ptr()) == 1
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process::{Child, Command};
    use std::thread::sleep;
    use std::time::{Duration, Instant};

    use super::*;

    /// A sleeping process, killed and reaped when dropped.
    struct Target(Child);

    impl Target {
        /// `/bin/sleep` under a profile that allows everything except reading a file and binding a
        /// socket inside `confined`.
        fn sandboxed(confined: &Path) -> Self {
            let profile = format!(
                "(version 1)(allow default)(deny file-read-data network-bind (subpath \"{}\"))",
                confined.display()
            );
            let target = Self(
                Command::new("/usr/bin/sandbox-exec")
                    .args(["-p", &profile, "/bin/sleep", "30"])
                    .spawn()
                    .expect("start a sandboxed sleeper"),
            );
            // sandbox-exec applies the profile before it execs, so wait until the kernel says so.
            let deadline = Instant::now() + Duration::from_secs(5);
            // SAFETY: a null operation with no filter asks only whether the process is sandboxed.
            while unsafe { sandbox_check(target.pid(), std::ptr::null(), 0) } != 1 {
                assert!(Instant::now() < deadline, "the sleeper never became sandboxed");
                sleep(Duration::from_millis(10));
            }
            target
        }

        fn unsandboxed() -> Self {
            Self(Command::new("/bin/sleep").arg("30").spawn().expect("start a sleeper"))
        }

        fn pid(&self) -> i32 {
            self.0.id() as i32
        }
    }

    impl Drop for Target {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// A helper file and an endpoint directory, each in either the confined or the open half of
    /// one temporary directory. Sandbox rules match resolved paths, so both halves are canonical.
    struct Layout {
        _root: tempfile::TempDir,
        confined: PathBuf,
        open: PathBuf,
    }

    impl Layout {
        fn new() -> Self {
            let root = tempfile::tempdir().expect("temporary layout");
            let base = fs::canonicalize(root.path()).expect("canonical layout");
            let (confined, open) = (base.join("confined"), base.join("open"));
            for directory in [&confined, &open] {
                fs::create_dir(directory).expect("layout directory");
                fs::create_dir(directory.join("endpoint")).expect("endpoint directory");
                fs::write(directory.join("recon-helper.dylib"), b"helper").expect("helper file");
            }
            Self { _root: root, confined, open }
        }

        fn helper(directory: &Path) -> PathBuf {
            directory.join("recon-helper.dylib")
        }

        fn socket(directory: &Path) -> PathBuf {
            directory.join("endpoint").join("sock")
        }
    }

    #[test]
    fn names_a_helper_the_target_sandbox_cannot_read() {
        let layout = Layout::new();
        let target = Target::sandboxed(&layout.confined);
        let helper = Layout::helper(&layout.confined);

        let refusal = load_refusal(target.pid(), &helper, &Layout::socket(&layout.open))
            .expect("the sandbox denies the helper");

        assert!(refusal.contains(&helper.display().to_string()), "{refusal}");
        assert!(!refusal.contains("endpoint"), "{refusal}");
    }

    #[test]
    fn names_an_endpoint_the_target_sandbox_cannot_create() {
        let layout = Layout::new();
        let target = Target::sandboxed(&layout.confined);
        let endpoint = Layout::socket(&layout.confined);

        let refusal = load_refusal(target.pid(), &Layout::helper(&layout.open), &endpoint)
            .expect("the sandbox denies the endpoint");

        let directory = endpoint.parent().expect("endpoint directory");
        assert!(refusal.contains(&directory.display().to_string()), "{refusal}");
        assert!(!refusal.contains("recon-helper.dylib"), "{refusal}");
    }

    #[test]
    fn allows_what_the_target_sandbox_permits() {
        let layout = Layout::new();
        let target = Target::sandboxed(&layout.confined);

        let refusal = load_refusal(
            target.pid(),
            &Layout::helper(&layout.open),
            &Layout::socket(&layout.open),
        );

        assert_eq!(refusal, None);
    }

    #[test]
    fn never_refuses_an_unsandboxed_target() {
        let layout = Layout::new();
        let target = Target::unsandboxed();

        let refusal = load_refusal(
            target.pid(),
            &Layout::helper(&layout.confined),
            &Layout::socket(&layout.confined),
        );

        assert_eq!(refusal, None);
    }
}
