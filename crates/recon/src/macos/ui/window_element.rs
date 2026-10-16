//! Finding the accessibility element of one window.
//!
//! An application reports only the windows on the current Space through `AXWindows`, so reaching a
//! window that is elsewhere needs the private remote-token construction, matched by window ID.

use std::ptr::NonNull;

use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::{CFArray, CFData, CFRetained, CFString};
use objc2_core_graphics::CGWindowID;

use super::accessibility::attribute;
use super::private;

type GetWindow = unsafe extern "C" fn(&AXUIElement, *mut CGWindowID) -> AXError;
type CreateWithRemoteToken = unsafe extern "C" fn(&CFData) -> *mut AXUIElement;

/// Element numbers have no documented upper bound; Recon scans the range winctl's probe covered.
const ELEMENT_NUMBERS: std::ops::RangeInclusive<u64> = 0..=99_999;

const WINDOW_ROLE: &str = "AXWindow";

/// The accessibility element of a window the instance owns.
pub fn window_element(
    application: &AXUIElement,
    pid: u32,
    window_id: u32,
) -> Result<CFRetained<AXUIElement>, String> {
    let get_window = window_id_lookup()?;

    if let Some(element) = listed_windows(application)
        .into_iter()
        .find(|element| window_id_of(element, get_window) == Some(window_id))
    {
        return Ok(element);
    }
    if let Some(element) = main_window(application)
        .filter(|element| window_id_of(element, get_window) == Some(window_id))
    {
        return Ok(element);
    }

    let Some(from_token) =
        private::function::<CreateWithRemoteToken>(c"_AXUIElementCreateWithRemoteToken")
    else {
        return Err(format!(
            "{} (remote-token lookup is unavailable on this macOS version)",
            unreachable_window(window_id)
        ));
    };
    scanned_window(pid, window_id, from_token, get_window)
        .ok_or_else(|| unreachable_window(window_id))
}

/// The window ID of the window the application reports as its main one.
pub fn main_window_id(application: &AXUIElement) -> Result<Option<u32>, String> {
    let get_window = window_id_lookup()?;
    Ok(main_window(application).and_then(|window| window_id_of(&window, get_window)))
}

fn window_id_lookup() -> Result<GetWindow, String> {
    private::function(c"_AXUIElementGetWindow").ok_or_else(|| {
        "accessibility window lookup is unavailable on this macOS version (_AXUIElementGetWindow \
         not found)"
            .to_owned()
    })
}

fn listed_windows(application: &AXUIElement) -> Vec<CFRetained<AXUIElement>> {
    let Some(windows) = attribute(application, "AXWindows") else {
        return Vec::new();
    };
    let Ok(windows) = windows.downcast::<CFArray>() else {
        return Vec::new();
    };
    // SAFETY: the windows of an application are accessibility elements.
    let windows: CFRetained<CFArray<AXUIElement>> = unsafe { CFRetained::cast_unchecked(windows) };
    windows.to_vec()
}

fn main_window(application: &AXUIElement) -> Option<CFRetained<AXUIElement>> {
    attribute(application, "AXMainWindow")?.downcast::<AXUIElement>().ok()
}

fn window_id_of(element: &AXUIElement, get_window: GetWindow) -> Option<u32> {
    let mut id: CGWindowID = 0;
    // SAFETY: the private lookup writes the window ID of an accessibility window.
    let error = unsafe { get_window(element, &raw mut id) };
    (error == AXError::Success).then_some(id)
}

fn scanned_window(
    pid: u32,
    window_id: u32,
    from_token: CreateWithRemoteToken,
    get_window: GetWindow,
) -> Option<CFRetained<AXUIElement>> {
    let pid = i32::try_from(pid).ok()?;
    ELEMENT_NUMBERS.filter_map(|number| element_from_token(pid, number, from_token)).find(
        |element| {
            window_id_of(element, get_window) == Some(window_id)
                && role_of(element).as_deref() == Some(WINDOW_ROLE)
        },
    )
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

fn role_of(element: &AXUIElement) -> Option<String> {
    Some(attribute(element, "AXRole")?.downcast::<CFString>().ok()?.to_string())
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

fn unreachable_window(window_id: u32) -> String {
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
    fn element_numbers_cover_the_documented_range() {
        assert_eq!(ELEMENT_NUMBERS.count(), 100_000);
        assert_eq!(remote_token(1, 99_999)[12..20], 99_999u64.to_le_bytes());
    }
}
