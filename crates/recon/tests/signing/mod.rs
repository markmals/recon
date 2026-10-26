use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::LazyLock;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub(super) const ORIGINAL: &str = "org.recon.tests.running-signature";
const REPLACEMENT: &str = "org.recon.tests.replacement-signature";

static LIBRARY: LazyLock<tempfile::TempDir> = LazyLock::new(|| {
    let scratch = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/signing/probe.c");
    let compiled = Command::new("cc")
        .args(["-dynamiclib", "-Wall", "-Wextra", "-Werror"])
        .arg(source)
        .args(["-framework", "Security", "-framework", "CoreFoundation", "-o"])
        .arg(scratch.path().join("probe.dylib"))
        .output()
        .unwrap();
    assert!(compiled.status.success(), "{compiled:?}");
    scratch
});

pub(super) struct Probe {
    scratch: tempfile::TempDir,
    executable: PathBuf,
}

pub(super) struct Invocation {
    pub(super) output: Output,
    pub(super) queries: Vec<Query>,
}

pub(super) struct Query {
    pub(super) pid: i32,
    pub(super) own_pid: i32,
    pub(super) operation: u32,
    pub(super) length: usize,
}

impl Probe {
    pub(super) fn new() -> Self {
        let scratch = tempfile::tempdir().unwrap();
        let executable = scratch.path().join("recon");
        fs::copy(env!("CARGO_BIN_EXE_recon"), &executable).unwrap();
        sign(&executable, ORIGINAL);
        Self { scratch, executable }
    }

    pub(super) fn run(&self, mode: &str, extra: &[(&str, String)]) -> Invocation {
        self.start(mode, extra).finish()
    }

    pub(super) fn replace(&self, unsigned: bool, mode: &str) -> Invocation {
        let replacement = self.scratch.path().join("replacement");
        fs::copy(env!("CARGO_BIN_EXE_recon"), &replacement).unwrap();
        if unsigned {
            let removed = Command::new("codesign")
                .arg("--remove-signature")
                .arg(&replacement)
                .output()
                .unwrap();
            assert!(removed.status.success(), "{removed:?}");
        } else {
            sign(&replacement, REPLACEMENT);
        }
        let ready = self.scratch.path().join("ready");
        let release = self.scratch.path().join("release");
        let mut ready = fifo(&ready);
        let mut release_writer = fifo(&release);
        let extra = [
            ("RECON_SIGNING_TEST_READY", self.scratch.path().join("ready").display().to_string()),
            ("RECON_SIGNING_TEST_RELEASE", release.display().to_string()),
        ];
        let mut running = self.start(mode, &extra);
        running.wait_ready(&mut ready);
        fs::rename(replacement, &self.executable).unwrap();
        release_writer.write_all(b"G").unwrap();
        running.finish()
    }

    fn start(&self, mode: &str, extra: &[(&str, String)]) -> Running {
        let trace = self.scratch.path().join("queries");
        let mut command = Command::new(&self.executable);
        command
            .arg("context")
            .env_remove("RECON_RESPONSIBILITY_DISCLAIMED")
            .env("DYLD_INSERT_LIBRARIES", LIBRARY.path().join("probe.dylib"))
            .env("RECON_SIGNING_TEST_MODE", mode)
            .env("RECON_SIGNING_TEST_TRACE", &trace)
            .envs(extra.iter().map(|(name, contents)| (name, contents)))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = command.spawn().unwrap();
        let stdout = reader(child.stdout.take().unwrap());
        let stderr = reader(child.stderr.take().unwrap());
        Running {
            child,
            stdout: Some(stdout),
            stderr: Some(stderr),
            trace,
            started: Instant::now(),
        }
    }
}

fn sign(path: &Path, identifier: &str) {
    let signed = Command::new("codesign")
        .args(["--force", "--sign", "-", "--identifier", identifier])
        .arg(path)
        .output()
        .unwrap();
    assert!(signed.status.success(), "{signed:?}");
}

fn fifo(path: &Path) -> File {
    let name = CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: the name is terminated and points into this test's owned temporary directory.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    OpenOptions::new().read(true).write(true).custom_flags(libc::O_NONBLOCK).open(path).unwrap()
}

fn reader(mut stream: impl Read + Send + 'static) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        bytes
    })
}

struct Running {
    child: Child,
    stdout: Option<JoinHandle<Vec<u8>>>,
    stderr: Option<JoinHandle<Vec<u8>>>,
    trace: PathBuf,
    started: Instant,
}

impl Running {
    fn wait_ready(&mut self, ready: &mut File) {
        let mut marker = [0];
        loop {
            match ready.read(&mut marker) {
                Ok(1) => {
                    assert_eq!(marker, *b"R");
                    return;
                }
                Ok(0) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                unexpected => panic!("signing barrier failed: {unexpected:?}"),
            }
            assert!(self.child.try_wait().unwrap().is_none(), "recon exited before its barrier");
            self.check_deadline();
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn check_deadline(&self) {
        assert!(self.started.elapsed() < Duration::from_secs(20), "signing probe timed out");
    }

    fn finish(mut self) -> Invocation {
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            self.check_deadline();
            thread::sleep(Duration::from_millis(10));
        };
        let stdout = self.stdout.take().unwrap().join().unwrap();
        let stderr = self.stderr.take().unwrap().join().unwrap();
        let trace = match fs::read_to_string(&self.trace) {
            Ok(trace) => trace,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => panic!("cannot read signing-query evidence: {error}"),
        };
        let queries = trace
            .lines()
            .map(|line| {
                let fields: Vec<_> = line.split_whitespace().collect();
                assert_eq!(fields.len(), 4, "{line}");
                Query {
                    pid: fields[0].parse().unwrap(),
                    own_pid: fields[1].parse().unwrap(),
                    operation: fields[2].parse().unwrap(),
                    length: fields[3].parse().unwrap(),
                }
            })
            .collect();
        Invocation { output: Output { status, stdout, stderr }, queries }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            // The root is still unreaped, so this owned process-group ID cannot have been reused.
            // SAFETY: only the isolated subprocess group created by this test is terminated.
            unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) };
            self.child.wait().unwrap();
        }
    }
}
