//! What this Mac says about Recon: how the running executable is signed, whether macOS attributes
//! Recon's privacy requests to Recon itself, which permissions it holds, which private interfaces
//! still exist, and whether its application discovery works.
//!
//! Permissions are preflighted and never requested. Signing queries read only this process;
//! other private symbols are looked up without calling them. No application is touched.

use std::ffi::CStr;

use objc2_foundation::NSProcessInfo;

use super::{permissions, private, responsibility, signing};
use crate::context::{
    ApplicationDiscovery, DiscoveryMechanism, InterfaceState, MacosFacts, PermissionState,
    ResponsibilityFacts,
};
use crate::error::Failure;
use crate::ui::permission::Permission;

const INTERFACES: [(&str, &CStr); 7] = [
    ("responsibility-query", c"responsibility_get_pid_responsible_for_pid"),
    ("responsibility-disclaim", c"responsibility_spawnattrs_setdisclaim"),
    ("code-signing-query", c"csops"),
    ("window-server-connection", c"CGSMainConnectionID"),
    ("skylight-capture", c"SLSHWCaptureWindowList"),
    ("accessibility-window-id", c"_AXUIElementGetWindow"),
    ("accessibility-remote-token", c"_AXUIElementCreateWithRemoteToken"),
];

/// Pre-spawn identity failures are reportable; a post-spawn failure owns the final response.
pub fn facts() -> Result<MacosFacts, Failure> {
    let responsibility = match responsibility::take_responsibility() {
        Ok(()) => ResponsibilityFacts { responsible_for_self: true, detail: None },
        Err(responsibility::ResponsibilityError::BeforeSpawn(failure)) => {
            ResponsibilityFacts { responsible_for_self: false, detail: Some(failure.message) }
        }
        Err(responsibility::ResponsibilityError::AfterSpawn(failure)) => return Err(failure),
    };
    let responsible = responsibility.responsible_for_self;

    Ok(MacosFacts {
        code_signing: signing::facts()?,
        responsibility,
        permissions: Permission::ALL
            .into_iter()
            .map(|permission| PermissionState {
                permission,
                // A preflight run under the launcher's identity answers a different question, so
                // an unknown grant is reported as unknown rather than as a denial.
                granted: responsible.then(|| permissions::granted(permission)),
                settings_pane: permission.settings_pane().to_owned(),
            })
            .collect(),
        interfaces: INTERFACES
            .into_iter()
            .map(|(interface, symbol)| InterfaceState {
                interface: interface.to_owned(),
                symbol: String::from_utf8_lossy(symbol.to_bytes()).into_owned(),
                available: private::available(symbol),
            })
            .collect(),
        application_discovery: application_discovery(),
    })
}

/// The macOS product version as `major.minor.patch`.
///
/// Read from the structured interface rather than from `operatingSystemVersionString`, which
/// Apple documents as unsuitable for parsing.
pub fn product_version() -> String {
    let version = NSProcessInfo::processInfo().operatingSystemVersion();
    format!("{}.{}.{}", version.majorVersion, version.minorVersion, version.patchVersion)
}

/// Whether Recon's own application discovery works here, measured by running the query
/// `recon app list` runs and counting what comes back.
fn application_discovery() -> ApplicationDiscovery {
    let count = crate::macos::indexed_application_count().ok();
    ApplicationDiscovery {
        mechanism: DiscoveryMechanism::Spotlight,
        available: count.is_some(),
        indexed_applications: count,
    }
}
