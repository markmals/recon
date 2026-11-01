//! What this Mac says about Recon: the facts its privacy identity carries, which private
//! interfaces each capability still finds, and whether its application discovery works.
//!
//! Permissions are preflighted and never requested. Other private symbols are looked up without
//! being called. No application is touched.

use std::ffi::CStr;
use std::ptr;

use recon_core::error::Failure;
use recon_privacy_macos::identity::PrivacyFacts;

use super::{ApplicationDiscovery, DiscoveryMechanism, InterfaceState, MacosFacts};

/// Pre-spawn identity failures are reportable; a post-spawn failure owns the final response.
pub(super) fn facts() -> Result<MacosFacts, Failure> {
    let PrivacyFacts { code_signing, responsibility, permissions } =
        recon_privacy_macos::identity::facts()?;
    Ok(MacosFacts {
        code_signing,
        responsibility,
        permissions,
        interfaces: interfaces(),
        application_discovery: application_discovery(),
    })
}

/// The macOS product version as `major.minor.patch`, or `None` when the kernel does not answer.
///
/// `kern.osproductversion` is the same number System Settings and `sw_vers` show, read without
/// parsing `operatingSystemVersionString`, which Apple documents as unsuitable for parsing. The
/// kernel leaves a trailing zero out — it answers `27.1` rather than `27.1.0` — so the missing
/// components are filled in rather than published as a shorter version than the contract
/// promises.
pub(super) fn product_version() -> Option<String> {
    let mut version = [0u8; 64];
    let mut length = version.len();
    // SAFETY: `kern.osproductversion` names a string the kernel writes into `version`, with
    // `length` carrying the buffer's size in and the answer's length out.
    let read = unsafe {
        libc::sysctlbyname(
            c"kern.osproductversion".as_ptr(),
            version.as_mut_ptr().cast(),
            &raw mut length,
            ptr::null_mut(),
            0,
        )
    };
    if read != 0 || length == 0 {
        return None;
    }
    let reported = CStr::from_bytes_until_nul(&version[..length]).ok()?.to_str().ok()?;
    let mut numbers = reported.split('.');
    let component = |number: Option<&str>| number.unwrap_or("0").to_owned();
    let (major, minor, patch) =
        (numbers.next()?.to_owned(), component(numbers.next()), component(numbers.next()));
    (!major.is_empty()).then(|| format!("{major}.{minor}.{patch}"))
}

/// Every private interface Recon depends on, reported in one fixed order: the privacy identity's
/// first, then the window server's, then accessibility's. Each capability owns the symbols it
/// actually resolves.
fn interfaces() -> Vec<InterfaceState> {
    recon_privacy_macos::interfaces()
        .into_iter()
        .chain(recon_window_server_macos::interfaces())
        .chain(recon_accessibility_macos::interfaces())
        .map(|(interface, symbol, available)| InterfaceState {
            interface: interface.to_owned(),
            symbol: symbol.to_owned(),
            available,
        })
        .collect()
}

/// Whether Recon's own application discovery works here, measured by running the query
/// `recon app list` runs and counting what comes back.
fn application_discovery() -> ApplicationDiscovery {
    let count = recon_applications_macos::discovery::indexed_application_count().ok();
    ApplicationDiscovery {
        mechanism: DiscoveryMechanism::Spotlight,
        available: count.is_some(),
        indexed_applications: count,
    }
}
