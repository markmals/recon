//! Finding the accessibility element of one window.
//!
//! An application reports only the windows on the current Space through `AXWindows`, so reaching a
//! window that is elsewhere needs the private remote-token construction, matched by window ID.

use std::ptr::NonNull;

use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::{CFArray, CFData, CFRetained, CFString};
use objc2_core_graphics::CGWindowID;

use super::accessibility::{MESSAGING_TIMEOUT, attribute};
use super::private;
use crate::application::Pid;
use crate::ui::window::WindowId;

type GetWindow = unsafe extern "C" fn(&AXUIElement, *mut CGWindowID) -> AXError;
type CreateWithRemoteToken = unsafe extern "C" fn(&CFData) -> *mut AXUIElement;

/// Element numbers have no documented upper bound; Recon scans the range winctl's probe covered.
const ELEMENT_NUMBERS: std::ops::RangeInclusive<u64> = 0..=99_999;

const WINDOW_ROLE: &str = "AXWindow";

/// The accessibility element of a window the instance owns.
pub(super) fn window_element(
    application: &AXUIElement,
    pid: Pid,
    window_id: WindowId,
) -> Result<CFRetained<AXUIElement>, String> {
    let get_window = window_id_lookup()?;

    for element in listed_windows(application, pid)? {
        if window_id_of(&element, get_window, pid)? == Some(window_id) {
            return Ok(element);
        }
    }
    if let Some(element) = main_window(application, pid)?
        && window_id_of(&element, get_window, pid)? == Some(window_id)
    {
        return Ok(element);
    }

    // SAFETY: `CreateWithRemoteToken` is the signature of `_AXUIElementCreateWithRemoteToken`.
    let Some(from_token) = (unsafe {
        private::function::<CreateWithRemoteToken>(c"_AXUIElementCreateWithRemoteToken")
    }) else {
        return Err(format!(
            "{} (remote-token lookup is unavailable on this macOS version)",
            unreachable_window(window_id)
        ));
    };
    scanned_window(pid, window_id, from_token, get_window)?
        .ok_or_else(|| unreachable_window(window_id))
}

/// The window ID of the window the application reports as its main one.
///
/// An application that reports no main window needs no window ID lookup, so it is told that it
/// has none rather than that a private interface is missing.
pub(super) fn main_window_id(
    application: &AXUIElement,
    pid: Pid,
) -> Result<Option<WindowId>, String> {
    let Some(window) = main_window(application, pid)? else {
        return Ok(None);
    };
    window_id_of(&window, window_id_lookup()?, pid)
}

/// What a lookup request that did not answer means for the search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lookup {
    /// This element is not the one, or has no such attribute: keep looking.
    Continue,
    /// The application did not answer within the messaging timeout. Every further request would
    /// wait just as long, so Recon stops rather than scan for hours.
    Unresponsive,
}

fn lookup_outcome(error: AXError) -> Lookup {
    match error {
        AXError::CannotComplete => Lookup::Unresponsive,
        _ => Lookup::Continue,
    }
}

fn keep_looking(error: AXError, pid: Pid) -> Result<(), String> {
    match lookup_outcome(error) {
        Lookup::Continue => Ok(()),
        Lookup::Unresponsive => Err(format!(
            "process {pid} did not answer an accessibility request within {MESSAGING_TIMEOUT} \
                 seconds"
        )),
    }
}

fn window_id_lookup() -> Result<GetWindow, String> {
    // SAFETY: `GetWindow` is the signature of `_AXUIElementGetWindow`.
    unsafe { private::function(c"_AXUIElementGetWindow") }.ok_or_else(|| {
        "accessibility window lookup is unavailable on this macOS version (_AXUIElementGetWindow \
         not found)"
            .to_owned()
    })
}

fn listed_windows(
    application: &AXUIElement,
    pid: Pid,
) -> Result<Vec<CFRetained<AXUIElement>>, String> {
    let windows = match attribute(application, "AXWindows") {
        Ok(windows) => windows,
        Err(error) => return keep_looking(error, pid).map(|()| Vec::new()),
    };
    let Ok(windows) = windows.downcast::<CFArray>() else {
        return Ok(Vec::new());
    };
    // SAFETY: the windows of an application are accessibility elements.
    let windows: CFRetained<CFArray<AXUIElement>> = unsafe { CFRetained::cast_unchecked(windows) };
    Ok(windows.to_vec())
}

fn main_window(
    application: &AXUIElement,
    pid: Pid,
) -> Result<Option<CFRetained<AXUIElement>>, String> {
    match attribute(application, "AXMainWindow") {
        Ok(window) => Ok(window.downcast::<AXUIElement>().ok()),
        Err(error) => keep_looking(error, pid).map(|()| None),
    }
}

fn window_id_of(
    element: &AXUIElement,
    get_window: GetWindow,
    pid: Pid,
) -> Result<Option<WindowId>, String> {
    let mut id: CGWindowID = 0;
    // SAFETY: the private lookup writes the window ID of an accessibility window.
    let error = unsafe { get_window(element, &raw mut id) };
    if error == AXError::Success {
        return Ok(Some(WindowId(id)));
    }
    keep_looking(error, pid).map(|()| None)
}

fn scanned_window(
    pid: Pid,
    window_id: WindowId,
    from_token: CreateWithRemoteToken,
    get_window: GetWindow,
) -> Result<Option<CFRetained<AXUIElement>>, String> {
    let token_pid = i32::try_from(pid.0).map_err(|_| format!("{pid} is not a process id"))?;
    for number in ELEMENT_NUMBERS {
        let Some(element) = element_from_token(token_pid, number, from_token) else {
            continue;
        };
        if window_id_of(&element, get_window, pid)? != Some(window_id) {
            continue;
        }
        if role_of(&element, pid)?.as_deref() == Some(WINDOW_ROLE) {
            return Ok(Some(element));
        }
    }
    Ok(None)
}

fn element_from_token(
    pid: i32,
    element_number: u64,
    from_token: CreateWithRemoteToken,
) -> Option<CFRetained<AXUIElement>> {
    let token = CFData::from_bytes(&remote_token(pid, element_number));
    // SAFETY: the private constructor returns an owned element, or null for an unused number.
    let element = NonNull::new(unsafe { from_token(&token) })?;
    // SAFETY: the returned element is owned by this process.
    Some(unsafe { CFRetained::from_raw(element) })
}

fn role_of(element: &AXUIElement, pid: Pid) -> Result<Option<String>, String> {
    match attribute(element, "AXRole") {
        Ok(role) => Ok(role.downcast::<CFString>().ok().map(|role| role.to_string())),
        Err(error) => keep_looking(error, pid).map(|()| None),
    }
}

/// The remote token of one element of one process: a process ID, a reserved word, the magic word
/// macOS expects, and the application's own element number, each little-endian.
fn remote_token(pid: i32, element_number: u64) -> [u8; 20] {
    const MAGIC: i32 = 0x636f_636f;

    let mut token = [0u8; 20];
    token[0..4].copy_from_slice(&pid.to_le_bytes());
    token[8..12].copy_from_slice(&MAGIC.to_le_bytes());
    token[12..20].copy_from_slice(&element_number.to_le_bytes());
    token
}

fn unreachable_window(window_id: WindowId) -> String {
    format!("window {window_id} has no accessibility element reachable by Recon")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remote_token_carries_the_process_the_magic_word_and_the_element_number() {
        let token = remote_token(412, 4236);

        assert_eq!(&token[0..4], &412i32.to_le_bytes());
        assert_eq!(&token[4..8], &[0, 0, 0, 0]);
        assert_eq!(i32::from_le_bytes(token[8..12].try_into().unwrap()), 0x636f_636f);
        assert_eq!(&token[12..20], &4236u64.to_le_bytes());
    }

    #[test]
    fn an_application_that_stops_answering_ends_the_search() {
        assert_eq!(lookup_outcome(AXError::CannotComplete), Lookup::Unresponsive);
        assert_eq!(
            keep_looking(AXError::CannotComplete, Pid(412)),
            Err("process 412 did not answer an accessibility request within 2 seconds".to_owned())
        );
    }

    #[test]
    fn an_element_that_is_absent_or_wrong_only_fails_to_match() {
        assert_eq!(lookup_outcome(AXError::Success), Lookup::Continue);
        assert_eq!(lookup_outcome(AXError::NoValue), Lookup::Continue);
        assert_eq!(lookup_outcome(AXError::AttributeUnsupported), Lookup::Continue);
        assert_eq!(lookup_outcome(AXError::InvalidUIElement), Lookup::Continue);
        assert_eq!(lookup_outcome(AXError::NotImplemented), Lookup::Continue);
    }
}
