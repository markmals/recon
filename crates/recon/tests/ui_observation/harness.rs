//! Launching the fixture, running the signed `recon`, and reading what the Mac shows while it
//! runs. Every observation here is made out of process, so that a failing test says whether Recon
//! disturbed the Mac rather than whether the test did.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use objc2_app_kit::{NSEvent, NSWorkspace};
use objc2_core_foundation::{
    CFArray, CFDictionary, CFNumber, CFRetained, CFRunLoop, CFString, CFType, kCFRunLoopDefaultMode,
};
use objc2_core_graphics::{
    CGDisplayCopyDisplayMode, CGDisplayMode, CGMainDisplayID, CGWindowListCopyWindowInfo,
    CGWindowListOption, kCGNullWindowID, kCGWindowOwnerPID,
};
use serde_json::Value;

pub const MAIN_WINDOW_TITLE: &str = "Recon Fixture — Main";
pub const INSPECTOR_WINDOW_TITLE: &str = "Recon Fixture — Inspector";

const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

/// One running instance of the fixture, launched in the background and terminated with the test.
pub struct Fixture {
    bundle: PathBuf,
    pub pid: u32,
}

impl Fixture {
    pub fn launch() -> Fixture {
        let bundle = fixture_bundle();
        let executable = bundle.join("Contents/MacOS/ReconFixture");
        assert!(
            running_fixture(&executable).is_none(),
            "a fixture instance is already running; quit it before running these tests"
        );

        let opened = Command::new("/usr/bin/open")
            .args(["-g".as_ref(), bundle.as_os_str()])
            .status()
            .expect("run open");
        assert!(opened.success(), "open -g {} failed", bundle.display());

        let pid = wait_for(|| running_fixture(&executable))
            .unwrap_or_else(|| panic!("{} did not start", bundle.display()));
        let fixture = Fixture { bundle, pid };
        wait_for(|| (window_count(pid) >= 2).then_some(()))
            .expect("the fixture did not open its windows");
        fixture
    }

    pub fn target(&self) -> &str {
        path_argument(&self.bundle)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = Command::new("/bin/kill").arg(self.pid.to_string()).status();
        let executable = self.bundle.join("Contents/MacOS/ReconFixture");
        wait_for(|| running_fixture(&executable).is_none().then_some(()));
    }
}

/// Runs a `ui` command that is expected to succeed and returns the JSON object it printed.
pub fn run_ui(arguments: &[&str]) -> Value {
    let mut invocation = vec!["ui"];
    invocation.extend_from_slice(arguments);
    let output = run_recon(&invocation);
    assert!(
        output.status.success(),
        "recon {} failed: {}",
        invocation.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("recon prints one JSON object on success")
}

/// Runs the signed `recon` and holds it to the quality bar every command shares: observation
/// leaves the frontmost application and the pointer where they were. A person using the Mac
/// during the run trips these checks too, so the suite must run hands-off.
pub fn run_recon(arguments: &[&str]) -> Output {
    let frontmost = frontmost_pid();
    assert!(frontmost.is_some(), "no application is frontmost; these checks need one to compare");
    let pointer = pointer_location();

    let output = Command::new(signed_recon()).args(arguments).output().expect("start recon");

    let invocation = arguments.join(" ");
    assert_eq!(
        frontmost_pid(),
        frontmost,
        "recon {invocation} changed the frontmost application (or someone switched apps during the run)"
    );
    assert_eq!(
        pointer_location(),
        pointer,
        "recon {invocation} moved the pointer (or someone moved the mouse during the run)"
    );
    output
}

pub fn frontmost_pid() -> Option<u32> {
    settle_workspace_notifications();
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()
        .and_then(|application| u32::try_from(application.processIdentifier()).ok())
}

pub fn main_window_id(fixture: &Fixture) -> u64 {
    let observation = run_ui(&["windows", fixture.target()]);
    window_titled(&observation, MAIN_WINDOW_TITLE)["id"].as_u64().expect("a window id")
}

pub fn window_titled<'a>(observation: &'a Value, title: &str) -> &'a Value {
    observation["windows"]
        .as_array()
        .expect("a windows array")
        .iter()
        .find(|window| window["title"] == title)
        .unwrap_or_else(|| panic!("no window titled {title} in {observation}"))
}

/// Every node of a tree result, root first.
pub fn nodes(root: &Value) -> Vec<&Value> {
    let mut found = vec![root];
    let mut index = 0;
    while index < found.len() {
        let node = found[index];
        index += 1;
        if let Some(children) = node["children"].as_array() {
            found.extend(children);
        }
    }
    found
}

pub fn png_pixel_size(path: &Path) -> (u64, u64) {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    assert!(
        bytes.len() > 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "{} is not a PNG",
        path.display()
    );
    let dimension = |offset: usize| {
        u64::from(u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("PNG header")))
    };
    (dimension(16), dimension(20))
}

/// The main display's backing pixels per point. Read from the display mode rather than
/// `NSScreen`, because libtest runs each test on a spawned thread and AppKit's screen list is
/// main-thread only.
pub fn backing_scale_factor() -> f64 {
    let mode = CGDisplayCopyDisplayMode(CGMainDisplayID()).expect("a main display mode");
    let points = CGDisplayMode::width(Some(&mode));
    let pixels = CGDisplayMode::pixel_width(Some(&mode));
    assert!(points > 0, "the main display reports no width");
    pixels as f64 / points as f64
}

pub fn path_argument(path: &Path) -> &str {
    path.to_str().expect("UTF-8 path")
}

pub struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    pub fn new() -> TemporaryDirectory {
        let unique =
            SystemTime::now().duration_since(UNIX_EPOCH).expect("a clock after 1970").as_nanos();
        let path = std::env::temp_dir().join(format!("recon-ui-observation-{unique}"));
        fs::create_dir(&path).expect("create a temporary directory");
        TemporaryDirectory(path)
    }

    pub fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn signed_recon() -> PathBuf {
    let recon = repository_target().join("signed/recon");
    assert!(recon.is_file(), "{} is missing; build it with `vp run recon:sign`", recon.display());
    recon
}

fn fixture_bundle() -> PathBuf {
    let bundle = repository_target().join("fixtures/ReconFixture.app");
    assert!(
        bundle.is_dir(),
        "{} is missing; build it with `vp run fixture:build`",
        bundle.display()
    );
    bundle.canonicalize().expect("canonical fixture path")
}

fn repository_target() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target")
}

/// `NSWorkspace` learns of activation changes through run-loop notifications; a test process that
/// never runs its run loop would keep reporting whichever application was frontmost at startup.
fn settle_workspace_notifications() {
    // SAFETY: an immutable Core Foundation constant.
    let mode = unsafe { kCFRunLoopDefaultMode };
    CFRunLoop::run_in_mode(mode, 0.1, false);
}

fn pointer_location() -> (f64, f64) {
    let location = NSEvent::mouseLocation();
    (location.x, location.y)
}

fn running_fixture(executable: &Path) -> Option<u32> {
    let found = Command::new("/usr/bin/pgrep")
        .args(["-f".as_ref(), executable.as_os_str()])
        .output()
        .expect("run pgrep");
    let pids: Vec<u32> = String::from_utf8(found.stdout)
        .expect("UTF-8 process ids")
        .lines()
        .map(|line| line.trim().parse().expect("a numeric process id"))
        .collect();
    assert!(pids.len() < 2, "several fixture instances are running: {pids:?}");
    pids.first().copied()
}

fn window_count(pid: u32) -> usize {
    let Some(windows) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, kCGNullWindowID)
    else {
        return 0;
    };
    // SAFETY: the window server describes each window with a dictionary keyed by CFString.
    let windows: CFRetained<CFArray<CFDictionary<CFString, CFType>>> =
        unsafe { CFRetained::cast_unchecked(windows) };
    // SAFETY: a documented, immutable dictionary key of `CGWindowListCopyWindowInfo`.
    let owner_key = unsafe { kCGWindowOwnerPID };
    windows.iter().filter(|window| window_owner(window, owner_key) == Some(pid)).count()
}

fn window_owner(window: &CFDictionary<CFString, CFType>, owner_key: &CFString) -> Option<u32> {
    let owner = window.get(owner_key)?.downcast::<CFNumber>().ok()?;
    owner.as_i32().and_then(|pid| u32::try_from(pid).ok())
}

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> Option<T> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    loop {
        if let Some(found) = probe() {
            return Some(found);
        }
        if Instant::now() >= deadline {
            return None;
        }
        sleep(Duration::from_millis(50));
    }
}
