//! The disturbances the guard has to report, caused deliberately from outside Recon.
//!
//! A guard that reports a quiet desktop is only worth something if the same guard fails when the
//! desktop is not quiet. Each control here makes one of the changes the contract names —
//! activation, window order, the pointer, the Space, the session's input, the frontmost
//! application's key window — while a Recon command runs, and puts back what it can afterwards.
//!
//! Every control works through an interface anyone could use, and none of them goes through
//! Recon.

use std::ffi::c_void;
use std::process::Command;
use std::ptr::NonNull;
use std::thread::sleep;
use std::time::{Duration, Instant};

use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_core_graphics::{CGEvent, CGEventSource, CGEventSourceStateID, CGEventType};

use crate::desktop::symbol;

/// `kCGEventSourceStateCombinedSessionState`: input as the session sees it.
const SESSION_STATE: i32 = 0;
/// `kCGEventSourceStateHIDSystemState`: input as the hardware delivers it.
const HID_STATE: i32 = 1;
/// `kCGSessionEventTap` and `kCGHIDEventTap`.
const SESSION_TAP: u32 = 1;
const HID_TAP: u32 = 0;
/// The virtual key code of `a` on every layout this evidence runs on. The control only has to
/// reach the session; which character it types is immaterial, and the receiver acts on none.
const KEY_A: u16 = 0;

type CreateSource = unsafe extern "C" fn(i32) -> *mut c_void;
type CreateKeyboardEvent = unsafe extern "C" fn(*mut c_void, u16, bool) -> *mut c_void;
type PostEvent = unsafe extern "C" fn(u32, *mut c_void);
type ReleaseType = unsafe extern "C" fn(*mut c_void);
type PreflightPost = unsafe extern "C" fn() -> bool;
type WarpPointer = unsafe extern "C" fn(objc2_core_foundation::CGPoint) -> i32;
type MainConnection = unsafe extern "C" fn() -> i32;
type CopyDisplaySpaces = unsafe extern "C" fn(i32) -> *mut CFArray<CFDictionary<CFString, CFType>>;
type SetCurrentSpace = unsafe extern "C" fn(i32, &CFString, i64) -> i32;

/// Brings another application forward and puts the receiver back, the way an automation that
/// activates a target and restores the desktop would.
pub(crate) fn activate_and_restore(target: &str, receiver: &str) {
    activate(target);
    sleep(Duration::from_millis(250));
    activate(receiver);
}

pub(crate) fn activate(bundle: &str) {
    let status = Command::new("/usr/bin/open").arg(bundle).status().expect("run open");
    assert!(status.success(), "open {bundle} failed");
}

/// An accessibility raise of the target's front window, performed by System Events, which is
/// the accessibility client macOS ships. Raising a window of the target is a window-order change
/// the guard must attribute to the target rather than to the person at the Mac.
///
/// The test process needs the Accessibility grant; a refusal is reported rather than passed off
/// as a quiet desktop. Calling the interface directly avoids a scripting process's startup time
/// moving the disturbance beyond the operation's watch interval.
pub(crate) fn ax_raise(pid: u32, window: u32) {
    let application =
        recon_accessibility_macos::element::application_element(recon_core::process::Pid(pid))
            .expect("an application element");
    let element = recon_accessibility_macos::window_element::window_element(
        &application,
        recon_core::process::Pid(pid),
        recon_window_server_macos::window::WindowId(window),
    )
    .expect("the target window element");
    let action = CFString::from_str("AXRaise");
    // SAFETY: this test deliberately performs AXRaise on the selected live target window.
    let status = unsafe { AXUIElementPerformAction(std::ptr::from_ref(&*element).cast(), &action) };
    assert_eq!(status, 0, "AXRaise failed with accessibility error {status}");
}

/// Moves the real pointer and puts it back, the way an automation that warps the cursor to click
/// and restores it would.
pub(crate) fn warp_pointer_and_restore() {
    // SAFETY: `WarpPointer` is the signature of `CGWarpMouseCursorPosition`, which moves the
    // cursor to the given global point.
    let warp: WarpPointer = unsafe { symbol(c"CGWarpMouseCursorPosition") }
        .expect("this Mac does not export CGWarpMouseCursorPosition");
    let origin = pointer();
    let moved = objc2_core_foundation::CGPoint { x: origin.x + 60.0, y: origin.y + 60.0 };
    // SAFETY: the resolved symbol takes one point by value.
    unsafe { warp(moved) };
    sleep(Duration::from_millis(200));
    // SAFETY: as above, putting the pointer back where it was.
    unsafe { warp(origin) };
    assert_ne!(
        (moved.x, moved.y),
        (origin.x, origin.y),
        "the control did not choose a different point"
    );
}

fn pointer() -> objc2_core_foundation::CGPoint {
    let event = CGEvent::new(None).expect("a Core Graphics event carrying the pointer location");
    CGEvent::location(Some(&event))
}

/// Whether this process may post events at all. Without the grant the input controls would post
/// nothing and report a quiet desktop, which is worse than failing.
pub(crate) fn can_post_events() -> bool {
    // SAFETY: `PreflightPost` is the signature of `CGPreflightPostEventAccess`, which reads the
    // grant without requesting it.
    match unsafe { symbol::<PreflightPost>(c"CGPreflightPostEventAccess") } {
        // SAFETY: the resolved symbol takes no arguments and only reads the grant.
        Some(preflight) => unsafe { preflight() },
        None => false,
    }
}

/// One keystroke posted into the session, as a script that types for the user would. Recon's
/// own posts are addressed to a process and never reach the session, so this is the input the
/// diverted-input check exists to catch.
pub(crate) fn session_keystroke() {
    assert!(
        post_keystroke(SESSION_STATE, SESSION_TAP),
        "this Mac does not export the Core Graphics event interfaces this control needs"
    );
}

/// Input as the hardware delivers it, for the one attribution rule a synthetic control can only
/// approximate: a change during hardware input is the person at the Mac, not the target.
///
/// Returns whether this Mac's hardware input counter actually moved. A Mac that counts a posted
/// HID-level event as hardware input makes this control possible; one that does not leaves the
/// row to a person at the keyboard, and the caller says so rather than claiming the evidence.
pub(crate) fn hardware_keystroke() -> bool {
    let before = CGEventSource::counter_for_event_type(
        CGEventSourceStateID::HIDSystemState,
        CGEventType::KeyDown,
    );
    if !post_keystroke(HID_STATE, HID_TAP) {
        return false;
    }
    sleep(Duration::from_millis(150));
    let after = CGEventSource::counter_for_event_type(
        CGEventSourceStateID::HIDSystemState,
        CGEventType::KeyDown,
    );
    if after.wrapping_sub(before) > 0 {
        return true;
    }
    eprintln!(
        "This Mac does not count a posted HID event as hardware input. Type one key now to \
         exercise the concurrent-user control."
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        sleep(Duration::from_millis(50));
        let hardware = CGEventSource::counter_for_event_type(
            CGEventSourceStateID::HIDSystemState,
            CGEventType::KeyDown,
        );
        if hardware != after {
            return true;
        }
    }
    false
}

/// A key down and a key up, 30 ms apart, from one event source into one tap.
fn post_keystroke(state: i32, tap: u32) -> bool {
    // SAFETY: each type is the signature of the Core Graphics function it names. The events are
    // created, posted and released here and shared with nothing.
    unsafe {
        let Some(create_source) = symbol::<CreateSource>(c"CGEventSourceCreate") else {
            return false;
        };
        let Some(create_event) = symbol::<CreateKeyboardEvent>(c"CGEventCreateKeyboardEvent")
        else {
            return false;
        };
        let Some(post) = symbol::<PostEvent>(c"CGEventPost") else {
            return false;
        };
        let Some(release) = symbol::<ReleaseType>(c"CFRelease") else {
            return false;
        };
        let source = create_source(state);
        for (index, down) in [true, false].into_iter().enumerate() {
            let event = create_event(source, KEY_A, down);
            if event.is_null() {
                if !source.is_null() {
                    release(source);
                }
                return false;
            }
            post(tap, event);
            release(event);
            if index == 0 {
                sleep(Duration::from_millis(30));
            }
        }
        if !source.is_null() {
            release(source);
        }
        true
    }
}

/// The Spaces of this Mac's main display, and the switch between two of them.
///
/// A Space change is one of the dimensions the guard keeps, and the only way to produce one
/// without a person at the keyboard is the window server's own interface.
pub(crate) struct Spaces {
    connection: i32,
    display: CFRetained<CFString>,
    current: i64,
    other: i64,
    set: SetCurrentSpace,
}

impl Spaces {
    /// Two Spaces of the main display, or nothing where this Mac has one Space or will not
    /// describe them.
    pub(crate) fn of_this_mac() -> Option<Spaces> {
        // SAFETY: each type is the signature of the window server symbol it is resolved with.
        // `CGSCopyManagedDisplaySpaces` follows the Core Foundation copy rule, so the array is
        // owned here.
        unsafe {
            let main = symbol::<MainConnection>(c"CGSMainConnectionID")?;
            let copy = symbol::<CopyDisplaySpaces>(c"CGSCopyManagedDisplaySpaces")?;
            let set = symbol::<SetCurrentSpace>(c"CGSManagedDisplaySetCurrentSpace")?;
            let connection = main();
            let displays: CFRetained<CFArray<CFDictionary<CFString, CFType>>> =
                CFRetained::from_raw(NonNull::new(copy(connection))?);
            let display = displays.get(0)?;
            let identifier = display
                .get(&CFString::from_str("Display Identifier"))?
                .downcast::<CFString>()
                .ok()?;
            let current = space_identifier(display.get(&CFString::from_str("Current Space"))?)?;
            let listed = display.get(&CFString::from_str("Spaces"))?.downcast::<CFArray>().ok()?;
            // SAFETY: the window server lists the Spaces of a display as dictionaries.
            let listed: CFRetained<CFArray<CFType>> = CFRetained::cast_unchecked(listed);
            let mut other = None;
            for index in 0..listed.len() {
                let identifier = space_identifier(listed.get(index)?)?;
                if identifier != current {
                    other = Some(identifier);
                    break;
                }
            }
            Some(Spaces { connection, display: identifier, current, other: other?, set })
        }
    }

    /// Switches to the other Space and comes back. Switching animates, so each half is given
    /// time to finish before the next one starts.
    pub(crate) fn switch_and_return(&self) {
        self.switch_to(self.other);
        sleep(Duration::from_millis(700));
        self.switch_to(self.current);
        sleep(Duration::from_millis(700));
    }

    fn switch_to(&self, space: i64) {
        // SAFETY: `SetCurrentSpace` is the signature of `CGSManagedDisplaySetCurrentSpace`,
        // which shows the given Space on the named display.
        let status = unsafe { (self.set)(self.connection, &self.display, space) };
        assert_eq!(status, 0, "the window server refused to show Space {space}");
    }
}

/// The `id64` of one Space description.
fn space_identifier(value: CFRetained<CFType>) -> Option<i64> {
    let dictionary = value.downcast::<CFDictionary>().ok()?;
    // SAFETY: the window server describes a Space with a dictionary keyed by CFString.
    let dictionary: CFRetained<CFDictionary<CFString, CFType>> =
        unsafe { CFRetained::cast_unchecked(dictionary) };
    dictionary.get(&CFString::from_str("id64"))?.downcast::<CFNumber>().ok()?.as_i64()
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementPerformAction(element: *const c_void, action: &CFString) -> i32;
}
