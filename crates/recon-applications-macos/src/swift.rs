//! The safe Rust side of this crate's Swift library.
//!
//! Every function here owns one call across the C ABI and nothing else: it converts arguments,
//! takes the string the Swift side allocated, copies it, and frees it through the one function
//! that may. No pointer the Swift side returned outlives the call that produced it.

use std::ffi::{CStr, CString, c_char};
use std::path::Path;

use serde_json::Value;

unsafe extern "C" {
    fn recon_applications_string_free(text: *mut c_char);
    fn recon_applications_running() -> *mut c_char;
    fn recon_applications_frontmost_pid() -> i32;
    fn recon_applications_display_name(path: *const c_char) -> *mut c_char;
    fn recon_applications_open(path: *const c_char, activates: bool, timeout: f64) -> *mut c_char;
}

/// Copies a string the Swift side allocated and releases it, or `None` when it returned nothing.
///
/// # Safety
///
/// `text` must be a pointer this crate's Swift library returned and has not yet been freed.
unsafe fn taken(text: *mut c_char) -> Option<String> {
    if text.is_null() {
        return None;
    }
    // SAFETY: the Swift side returns a NUL-terminated UTF-8 string it allocated for us.
    let owned = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
    // SAFETY: the same pointer, freed once, through the only function that may free it.
    unsafe { recon_applications_string_free(text) };
    Some(owned)
}

/// Every running application that has a bundle, as `(bundle path, process id)`.
pub(crate) fn running_applications() -> Vec<(String, u32)> {
    // SAFETY: the call takes no arguments and returns a string this process then owns.
    let Some(reported) = (unsafe { taken(recon_applications_running()) }) else {
        return Vec::new();
    };
    let Ok(Value::Array(entries)) = serde_json::from_str::<Value>(&reported) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let path = entry.get("path")?.as_str()?.to_owned();
            let pid = u32::try_from(entry.get("pid")?.as_i64()?).ok()?;
            Some((path, pid))
        })
        .collect()
}

/// The process ID of the frontmost application, or `None` when macOS reports none.
pub(crate) fn frontmost_application() -> Option<u32> {
    // SAFETY: the call takes no arguments and returns a plain integer.
    u32::try_from(unsafe { recon_applications_frontmost_pid() }).ok()
}

/// The display name macOS shows for `path`, and `None` when it has none or the path is not
/// representable as a C string.
pub(crate) fn display_name(path: &Path) -> Option<String> {
    let path = CString::new(path.to_str()?).ok()?;
    // SAFETY: `path` outlives the call, and the result is a string this process then owns.
    let name = unsafe { taken(recon_applications_display_name(path.as_ptr())) }?;
    (!name.is_empty()).then_some(name)
}

/// What macOS did when asked to open one application bundle.
pub(crate) enum Opened {
    /// The process ID of the instance macOS opened or reused.
    Pid(u32),
    /// The callback did not arrive inside the budget the caller allowed.
    TimedOut,
    /// The named step that did not happen, with whatever macOS said about it.
    Refused { reason: String, detail: Option<String> },
    /// The Swift side answered something this version cannot read.
    Unreadable(String),
}

/// Opens exactly `path`, optionally activating it, waiting at most `timeout_seconds` for the
/// answer; a negative timeout waits indefinitely.
pub(crate) fn open_application(path: &Path, activates: bool, timeout_seconds: f64) -> Opened {
    let Some(text) = path.to_str() else {
        return Opened::Unreadable("the application path is not UTF-8".to_owned());
    };
    let Ok(path) = CString::new(text) else {
        return Opened::Unreadable("the application path contains a NUL byte".to_owned());
    };
    // SAFETY: `path` outlives the call, and the result is a string this process then owns.
    let reported =
        unsafe { taken(recon_applications_open(path.as_ptr(), activates, timeout_seconds)) };
    let Some(reported) = reported else {
        return Opened::Unreadable("macOS reported no outcome at all".to_owned());
    };
    outcome(&reported)
}

fn outcome(reported: &str) -> Opened {
    let Ok(reported) = serde_json::from_str::<Value>(reported) else {
        return Opened::Unreadable(format!("macOS reported {reported}"));
    };
    if let Some(pid) = reported.get("pid").and_then(Value::as_i64) {
        return match u32::try_from(pid) {
            Ok(pid) => Opened::Pid(pid),
            Err(_) => Opened::Refused { reason: "no-process-id".to_owned(), detail: None },
        };
    }
    match reported.get("reason").and_then(Value::as_str) {
        Some("timeout") => Opened::TimedOut,
        Some(reason) => Opened::Refused {
            reason: reason.to_owned(),
            detail: reported.get("detail").and_then(Value::as_str).map(str::to_owned),
        },
        None => Opened::Unreadable(format!("macOS reported {reported}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_opened_application_is_reported_by_its_process_id() {
        assert!(matches!(outcome(r#"{"pid":412}"#), Opened::Pid(412)));
    }

    #[test]
    fn each_refusal_keeps_the_step_that_did_not_happen_and_what_macos_said() {
        let Opened::Refused { reason, detail } =
            outcome(r#"{"reason":"refused","detail":"The application is damaged"}"#)
        else {
            panic!("a refusal");
        };
        assert_eq!(reason, "refused");
        assert_eq!(detail.as_deref(), Some("The application is damaged"));

        assert!(matches!(
            outcome(r#"{"reason":"not-opened"}"#),
            Opened::Refused { detail: None, .. }
        ));
        assert!(matches!(outcome(r#"{"reason":"timeout"}"#), Opened::TimedOut));
    }

    #[test]
    fn an_answer_this_version_cannot_read_is_never_a_successful_launch() {
        for reported in ["", "{}", "not json", r#"{"pid":"412"}"#] {
            assert!(matches!(outcome(reported), Opened::Unreadable(_)), "{reported}");
        }
    }
}
