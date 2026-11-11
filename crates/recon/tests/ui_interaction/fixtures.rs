//! The three applications an interaction run needs, and the records they keep.
//!
//! Two instrumented targets — an AppKit one and a real Mac Catalyst one, each with two
//! independently instrumented windows — and a foreground receiver that holds the user's place:
//! it stays frontmost, keeps the keyboard focus, and covers the targets. Every claim about what
//! an operation did comes from the target's own record of what reached it, read out of process,
//! and every claim about the user's work comes from the receiver's.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};

use recon_core::process::Pid;
use recon_window_server_macos::inventory::windows_owned_by;
use serde_json::Value;

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
/// How long a record may keep growing before it is read. A posted event runs its handler after
/// Recon has returned, so the record is read once it has stopped changing rather than at once.
const QUIET: Duration = Duration::from_millis(400);
const SETTLE_TIMEOUT: Duration = Duration::from_secs(3);

/// One launched application and the record it keeps.
pub(crate) struct App {
    bundle: PathBuf,
    executable: PathBuf,
    name: &'static str,
    pub(crate) pid: u32,
}

impl App {
    /// The instrumented AppKit target, already running without any test-driven activation.
    pub(crate) fn appkit() -> App {
        App::from_running("ReconFixture", 2)
    }

    /// The instrumented Mac Catalyst target, already running without any test-driven activation.
    /// Its two windows are two scenes: one WindowServer window hosts one `UIWindow`.
    pub(crate) fn catalyst() -> App {
        App::from_running("ReconCatalystFixture", 2)
    }

    /// The application the person at the Mac is using, already running and frontmost.
    pub(crate) fn receiver() -> App {
        App::from_running("ReconForegroundReceiver", 2)
    }

    fn from_running(name: &'static str, windows: usize) -> App {
        let bundle = bundle(name);
        let executable = bundle.join("Contents/MacOS").join(name);
        let pid = running(&executable).unwrap_or_else(|| {
            panic!(
                "{name} is not running; start {} yourself before running the interaction evidence",
                bundle.display()
            )
        });
        let app = App { bundle, executable, name, pid };
        wait_for(|| (window_count(pid) >= windows).then_some(()))
            .unwrap_or_else(|| panic!("{name} did not open {windows} windows"));
        wait_for(|| {
            let evidence = app.read()?;
            (evidence.windows().len() >= windows).then_some(())
        })
        .unwrap_or_else(|| {
            panic!(
                "{name} did not publish {windows} instrumented windows at {}",
                app.record().display()
            )
        });
        app
    }

    /// The target as every `recon ui` command names it: the absolute bundle path.
    pub(crate) fn target(&self) -> &str {
        self.bundle.to_str().expect("UTF-8 bundle path")
    }

    /// The record as it stands now.
    pub(crate) fn evidence(&self) -> Evidence {
        self.read().unwrap_or_else(|| {
            panic!("{} keeps no record at {}", self.name, self.record().display())
        })
    }

    /// The record once it has stopped growing: an event posted to a target runs its handler on
    /// the target's own main thread, which can be after Recon has returned.
    pub(crate) fn settled(&self) -> Evidence {
        let deadline = Instant::now() + SETTLE_TIMEOUT;
        let mut evidence = self.evidence();
        let mut quiet_since = Instant::now();
        loop {
            sleep(Duration::from_millis(25));
            let current = self.evidence();
            if current.sequence() != evidence.sequence() {
                quiet_since = Instant::now();
            }
            evidence = current;
            if quiet_since.elapsed() >= QUIET || Instant::now() >= deadline {
                return evidence;
            }
        }
    }

    /// Asks this application to do something to itself, and waits for it to report that it did.
    /// Only the frontmost application can change the frontmost application's key window, which
    /// is one of the disturbances the guard has to report.
    pub(crate) fn perform(&self, command: &str) {
        let mark = self.evidence().sequence();
        fs::write(self.command_file(), command)
            .unwrap_or_else(|error| panic!("write {}: {error}", self.command_file().display()));
        let acted = wait_for(|| {
            let evidence = self.evidence();
            evidence
                .since(mark)
                .iter()
                .any(|event| event["kind"] == "command" && event["command"] == command)
                .then_some(())
        });
        assert!(acted.is_some(), "{} never carried out {command}", self.name);
    }

    fn read(&self) -> Option<Evidence> {
        let text = fs::read_to_string(self.record()).ok()?;
        let value: Value = serde_json::from_str(&text).ok()?;
        Some(Evidence { value })
    }

    fn record(&self) -> PathBuf {
        evidence_directory().join(format!("{}-{}.json", self.name, self.pid))
    }

    fn command_file(&self) -> PathBuf {
        evidence_directory().join(format!("{}-{}.command", self.name, self.pid))
    }
}

/// One application's record of what reached it.
pub(crate) struct Evidence {
    value: Value,
}

impl Evidence {
    pub(crate) fn pid(&self) -> u32 {
        u32::try_from(self.value["pid"].as_u64().expect("a recorded pid")).expect("a pid")
    }

    pub(crate) fn toolkit(&self) -> &str {
        self.value["toolkit"].as_str().expect("a recorded toolkit")
    }

    pub(crate) fn windows(&self) -> &[Value] {
        self.value["windows"].as_array().map(Vec::as_slice).unwrap_or_default()
    }

    /// One instrumented window of this application, by the label it published.
    pub(crate) fn window(&self, label: &str) -> &Value {
        self.windows()
            .iter()
            .find(|window| window["label"] == label)
            .unwrap_or_else(|| panic!("no window labelled {label} in {}", self.value))
    }

    /// The sequence number of the last event recorded, which marks a point in the record.
    pub(crate) fn sequence(&self) -> u64 {
        self.events().last().and_then(|event| event["sequence"].as_u64()).unwrap_or(0)
    }

    pub(crate) fn events(&self) -> &[Value] {
        self.value["events"].as_array().map(Vec::as_slice).unwrap_or_default()
    }

    /// Everything recorded after a mark.
    pub(crate) fn since(&self, mark: u64) -> Vec<Value> {
        self.events()
            .iter()
            .filter(|event| event["sequence"].as_u64().unwrap_or(0) > mark)
            .cloned()
            .collect()
    }
}

/// The events of one window, in order.
pub(crate) fn in_window<'a>(events: &'a [Value], window: &str) -> Vec<&'a Value> {
    events.iter().filter(|event| event["window"] == window).collect()
}

/// The events of one window of one kind, in order.
pub(crate) fn of_kind<'a>(events: &'a [Value], window: &str, kind: &str) -> Vec<&'a Value> {
    events.iter().filter(|event| event["window"] == window && event["kind"] == kind).collect()
}

/// Where a published control is in the window Recon addresses, in window-local points from the
/// top left of the window's bounds, frame included.
///
/// A fixture publishes its controls in its own window's coordinates, which are the content
/// area's: the title bar above it, and on Catalyst the interface scale, are the difference
/// between that and the window the window server reports. Both are derived from the two
/// rectangles rather than assumed, so an interface drawn at another scale moves the point
/// instead of silently missing the control.
pub(crate) fn control_point(window: &Value, bounds: &Value, control: &str) -> (f64, f64) {
    let number = |value: &Value, key: &str| {
        value[key].as_f64().unwrap_or_else(|| panic!("no {key} in {value}"))
    };
    let content_width = number(window, "contentWidth");
    let content_height = number(window, "contentHeight");
    let width = number(bounds, "width");
    let height = number(bounds, "height");
    assert!(content_width > 0.0 && content_height > 0.0, "{window}");
    let scale = width / content_width;
    assert!(
        (0.5..=2.0).contains(&scale),
        "the window server reports {width}×{height} for a {content_width}×{content_height} \
         interface; that is not a scale a point can be converted through: {window}"
    );
    let top = height - content_height * scale;
    assert!(top >= 0.0, "a window smaller than its own content: {window} against {bounds}");
    let rect = window["controls"]
        .as_array()
        .expect("published controls")
        .iter()
        .find(|published| published["name"] == control)
        .unwrap_or_else(|| panic!("no control named {control} in {window}"));
    let x = (number(rect, "x") + number(rect, "width") / 2.0) * scale;
    let y = (number(rect, "y") + number(rect, "height") / 2.0) * scale + top;
    assert!(
        x > 0.0 && x < width && y > top && y < height,
        "the centre of {control} is not inside the window: ({x}, {y}) in {width}×{height}"
    );
    (x, y)
}

/// A coordinate as the command line takes it.
pub(crate) fn coordinate(value: f64) -> String {
    format!("{value:.0}")
}

pub(crate) fn bundle(name: &str) -> PathBuf {
    let bundle = fixtures_directory().join(format!("{name}.app"));
    assert!(
        bundle.is_dir(),
        "{} is missing; build it with `mise run fixture:build`, `mise run fixture:catalyst` or \
         `mise run fixture:receiver`",
        bundle.display()
    );
    bundle.canonicalize().expect("canonical bundle path")
}

fn fixtures_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/fixtures")
}

/// Where the applications write their records: beside the bundles they were built into. The
/// path is canonical, because a launched application derives the same directory from its own
/// bundle location rather than from this one.
fn evidence_directory() -> PathBuf {
    let fixtures = fixtures_directory();
    fixtures.canonicalize().unwrap_or(fixtures).join("evidence")
}

fn running(executable: &Path) -> Option<u32> {
    let found = Command::new("/usr/bin/pgrep")
        .args(["-f".as_ref(), executable.as_os_str()])
        .output()
        .expect("run pgrep");
    let pids: Vec<u32> = String::from_utf8(found.stdout)
        .expect("UTF-8 process ids")
        .lines()
        .map(|line| line.trim().parse().expect("a numeric process id"))
        .collect();
    assert!(pids.len() < 2, "several instances are running: {pids:?}");
    pids.first().copied()
}

fn window_count(pid: u32) -> usize {
    windows_owned_by(Pid(pid)).map(|windows| windows.len()).unwrap_or_default()
}

pub(crate) fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> Option<T> {
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
