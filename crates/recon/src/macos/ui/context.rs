//! What this Mac says about Recon: how the running executable is signed, whether macOS attributes
//! Recon's privacy requests to Recon itself, which permissions it holds, which private interfaces
//! still exist, and whether its application discovery works.
//!
//! Everything here reads state. Permissions are preflighted and never requested, symbols are
//! looked up and never called, and no application is touched.

use std::ffi::{CStr, c_void};
use std::ptr::{self, NonNull};

use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_foundation::NSProcessInfo;
use objc2_security::{
    SecCSFlags, SecCode, SecCodeSignatureFlags, SecStaticCode, errSecCSUnsigned, errSecSuccess,
    kSecCSSigningInformation, kSecCodeInfoFlags, kSecCodeInfoIdentifier,
    kSecCodeInfoTeamIdentifier,
};

use super::{permissions, private, responsibility};
use crate::context::{
    ApplicationDiscovery, CodeSigningFacts, DiscoveryMechanism, InterfaceState, MacosFacts,
    PermissionState, ResponsibilityFacts,
};
use crate::error::Failure;
use crate::ui::permission::Permission;

/// Every private interface Recon depends on, paired with the symbol whose presence decides it.
/// A macOS update that removes one makes the capability unavailable rather than making Recon
/// unable to start, so what Recon can reach is worth reporting on its own.
const INTERFACES: [(&str, &CStr); 6] = [
    ("responsibility-query", c"responsibility_get_pid_responsible_for_pid"),
    ("responsibility-disclaim", c"responsibility_spawnattrs_setdisclaim"),
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
        code_signing: code_signing()?,
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

/// How the running executable is signed.
///
/// macOS distinguishes code that carries no signature from code whose signature it could not
/// read, and so does this: the first is the answer `signed: false`, and the second is a failure,
/// because reporting an unreadable signature as an absent one would be a claim Recon cannot make.
pub(super) fn code_signing() -> Result<CodeSigningFacts, Failure> {
    let Some(information) = signing_information()? else {
        return Ok(CodeSigningFacts {
            signed: false,
            identifier: None,
            team_identifier: None,
            ad_hoc: false,
        });
    };

    // SAFETY: each key is the documented static CFString for the information it names.
    let (identifier, team_identifier) = unsafe {
        (
            string(&information, kSecCodeInfoIdentifier),
            string(&information, kSecCodeInfoTeamIdentifier),
        )
    };
    // `kSecCodeInfoIdentifier` is present whenever code is signed and absent whenever it is not.
    Ok(CodeSigningFacts {
        signed: identifier.is_some(),
        identifier,
        team_identifier,
        ad_hoc: ad_hoc(&information),
    })
}

/// The signing information of the running process, or `None` when macOS reports that this code
/// carries no signature at all.
fn signing_information() -> Result<Option<CFRetained<CFDictionary>>, Failure> {
    let mut code: *mut SecCode = ptr::null_mut();
    // SAFETY: the out parameter is a live local.
    let status = unsafe { SecCode::copy_self(SecCSFlags::empty(), NonNull::from(&mut code)) };
    let Some(code) = copied("SecCodeCopySelf", status, code.cast_const())? else {
        return Ok(None);
    };
    // SAFETY: a successful copy returns an owned reference.
    let code = unsafe { CFRetained::from_raw(code) };

    let mut on_disk: *const SecStaticCode = ptr::null();
    // SAFETY: the out parameter is a live local.
    let status = unsafe { code.copy_static_code(SecCSFlags::empty(), NonNull::from(&mut on_disk)) };
    let Some(on_disk) = copied("SecCodeCopyStaticCode", status, on_disk)? else {
        return Ok(None);
    };
    // SAFETY: a successful copy returns an owned reference.
    let on_disk = unsafe { CFRetained::from_raw(on_disk) };

    let mut information: *const CFDictionary = ptr::null();
    // SAFETY: the static code is live and the out parameter is a live local.
    let status = unsafe {
        SecCode::copy_signing_information(
            &on_disk,
            SecCSFlags(kSecCSSigningInformation),
            NonNull::from(&mut information),
        )
    };
    let Some(information) = copied("SecCodeCopySigningInformation", status, information)? else {
        return Ok(None);
    };
    // SAFETY: a successful copy returns an owned reference.
    Ok(Some(unsafe { CFRetained::from_raw(information) }))
}

/// What one code-signing call returned: the reference it copied, nothing because this code is
/// unsigned, or a failure Recon did not anticipate and will not describe as an unsigned binary.
fn copied<T>(call: &str, status: i32, reference: *const T) -> Result<Option<NonNull<T>>, Failure> {
    // The one failure that is an answer: macOS reports this for code that carries no signature.
    if status == errSecCSUnsigned {
        return Ok(None);
    }
    match NonNull::new(reference.cast_mut()) {
        Some(reference) if status == errSecSuccess => Ok(Some(reference)),
        _ => Err(Failure::internal(format!(
            "{call} could not report how this recon is signed (OSStatus {status})"
        ))),
    }
}

/// Whether the signature is ad-hoc, which means its designated requirement is the code hash and
/// a rebuild loses every grant recorded against it.
fn ad_hoc(information: &CFDictionary) -> bool {
    // SAFETY: the key is the documented static CFString for the signature flags.
    let flags = value(information, unsafe { kSecCodeInfoFlags })
        .and_then(|flags| flags.downcast_ref::<CFNumber>())
        .and_then(CFNumber::as_i64)
        .and_then(|flags| u32::try_from(flags).ok());
    flags.is_some_and(|flags| {
        SecCodeSignatureFlags::from_bits_retain(flags).contains(SecCodeSignatureFlags::Adhoc)
    })
}

fn string(information: &CFDictionary, key: &CFString) -> Option<String> {
    value(information, key)
        .and_then(|value| value.downcast_ref::<CFString>())
        .map(CFString::to_string)
}

fn value<'a>(information: &'a CFDictionary, key: &CFString) -> Option<&'a CFType> {
    // SAFETY: the key is a CFString, which is what this dictionary is keyed by.
    let value = unsafe { information.value((key as *const CFString).cast::<c_void>()) };
    let value = NonNull::new(value.cast_mut())?;
    // SAFETY: the dictionary owns the value and outlives the borrow.
    Some(unsafe { value.cast::<CFType>().as_ref() })
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
