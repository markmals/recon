//! What this Mac says about Recon: the facts its privacy identity carries, which private
//! interfaces each capability still finds, and whether its application discovery works.
//!
//! Permissions are preflighted and never requested. Other private symbols are looked up without
//! being called. No application is touched.

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

/// The macOS product version as `major.minor.patch`.
///
/// Read from the structured interface rather than from `operatingSystemVersionString`, which
/// Apple documents as unsuitable for parsing.
pub(super) fn product_version() -> String {
    let version = objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion();
    format!("{}.{}.{}", version.majorVersion, version.minorVersion, version.patchVersion)
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
