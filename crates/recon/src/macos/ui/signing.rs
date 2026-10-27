use std::ffi::c_void;
use std::mem::{size_of, size_of_val};
use std::ptr::{self, NonNull};

use objc2_core_foundation::{CFData, CFDictionary, CFNumber, CFRetained, CFString, CFType};
use objc2_security::{
    SecCSFlags, SecCode, SecCodeSignatureFlags, SecStaticCode, errSecSuccess,
    kSecCSSigningInformation, kSecCodeInfoFlags, kSecCodeInfoIdentifier,
    kSecCodeInfoTeamIdentifier, kSecCodeInfoUnique,
};

use super::private;
use crate::context::CodeSigningFacts;
use crate::error::Failure;

// XNU bsd/sys/codesign.h and osfmk/kern/cs_blobs.h; CS_SIGNED does not imply CS_VALID.
const CS_OPS_STATUS: u32 = 0;
const CS_OPS_CDHASH: u32 = 5;
const CS_SIGNED: u32 = 0x2000_0000;
const CDHASH_LENGTH: usize = 20;

type CsOps = unsafe extern "C" fn(libc::pid_t, u32, *mut c_void, usize) -> libc::c_int;

pub(super) fn facts() -> Result<CodeSigningFacts, Failure> {
    let Some(kernel_hash) = kernel_signature()? else {
        return Ok(CodeSigningFacts {
            signed: false,
            identifier: None,
            team_identifier: None,
            ad_hoc: false,
        });
    };
    let information = signing_information()?;
    bind_selected_metadata(&information, &kernel_hash)?;

    // SAFETY: these are the documented static CFString keys for signing information.
    let (identifier, team_identifier) = unsafe {
        (
            string(&information, kSecCodeInfoIdentifier)?.ok_or_else(invalid_metadata)?,
            string(&information, kSecCodeInfoTeamIdentifier)?,
        )
    };
    Ok(CodeSigningFacts {
        signed: true,
        identifier: Some(identifier),
        team_identifier,
        ad_hoc: ad_hoc(&information)?,
    })
}

fn kernel_signature() -> Result<Option<[u8; CDHASH_LENGTH]>, Failure> {
    // SAFETY: CsOps matches the csops declaration in XNU bsd/sys/codesign.h.
    let query: CsOps = unsafe { private::function(c"csops") }
        .ok_or_else(|| Failure::internal("csops is unavailable for this recon's signing facts"))?;
    // SAFETY: getpid reads only the current process ID.
    let pid = unsafe { libc::getpid() };
    let mut status = 0_u32;
    // SAFETY: STATUS writes a u32 to this live, correctly sized local for our own PID.
    if unsafe { query(pid, CS_OPS_STATUS, ptr::from_mut(&mut status).cast(), size_of::<u32>()) }
        != 0
    {
        return Err(Failure::internal(format!(
            "csops could not read this recon's signing status: {}",
            std::io::Error::last_os_error()
        )));
    }
    if status & CS_SIGNED == 0 {
        return Ok(None);
    }
    let mut hash = [0_u8; CDHASH_LENGTH];
    // SAFETY: CDHASH writes 20 bytes to this live, correctly sized local for our own PID.
    if unsafe { query(pid, CS_OPS_CDHASH, hash.as_mut_ptr().cast(), size_of_val(&hash)) } != 0 {
        return Err(Failure::internal(format!(
            "csops could not read this recon's signing hash: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(Some(hash))
}

fn bind_selected_metadata(
    information: &CFDictionary,
    kernel_hash: &[u8; CDHASH_LENGTH],
) -> Result<(), Failure> {
    // SAFETY: the key names the selected CodeDirectory's unique hash, not alternate hashes.
    let selected = value(information, unsafe { kSecCodeInfoUnique })
        .and_then(|hash| hash.downcast_ref::<CFData>())
        .filter(|hash| hash.length() == CDHASH_LENGTH as isize)
        .ok_or_else(invalid_metadata)?;
    // SAFETY: the signing information owns this immutable CFData with the checked byte length.
    let selected = unsafe { std::slice::from_raw_parts(selected.byte_ptr(), CDHASH_LENGTH) };
    if selected != kernel_hash {
        return Err(Failure::internal("Signing metadata does not describe this running recon"));
    }
    Ok(())
}

fn signing_information() -> Result<CFRetained<CFDictionary>, Failure> {
    let mut code: *mut SecCode = ptr::null_mut();
    // SAFETY: the out parameter is a live local.
    let status = unsafe { SecCode::copy_self(SecCSFlags::empty(), NonNull::from(&mut code)) };
    let code = copied("SecCodeCopySelf", status, code.cast_const())?;
    // SAFETY: a successful copy returns an owned reference.
    let code = unsafe { CFRetained::from_raw(code) };

    let mut on_disk: *const SecStaticCode = ptr::null();
    // SAFETY: the out parameter is a live local.
    let status = unsafe { code.copy_static_code(SecCSFlags::empty(), NonNull::from(&mut on_disk)) };
    let on_disk = copied("SecCodeCopyStaticCode", status, on_disk)?;
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
    let information = copied("SecCodeCopySigningInformation", status, information)?;
    // SAFETY: a successful copy returns an owned reference.
    Ok(unsafe { CFRetained::from_raw(information) })
}

fn copied<T>(call: &str, status: i32, reference: *const T) -> Result<NonNull<T>, Failure> {
    match NonNull::new(reference.cast_mut()) {
        Some(reference) if status == errSecSuccess => Ok(reference),
        _ => Err(Failure::internal(format!(
            "{call} could not report how this recon is signed (OSStatus {status})"
        ))),
    }
}

fn ad_hoc(information: &CFDictionary) -> Result<bool, Failure> {
    // SAFETY: the key is the documented static CFString for the signature flags.
    let flags = value(information, unsafe { kSecCodeInfoFlags })
        .and_then(|flags| flags.downcast_ref::<CFNumber>())
        .and_then(CFNumber::as_i64)
        .and_then(|flags| u32::try_from(flags).ok())
        .ok_or_else(invalid_metadata)?;
    Ok(SecCodeSignatureFlags::from_bits_retain(flags).contains(SecCodeSignatureFlags::Adhoc))
}

fn string(information: &CFDictionary, key: &CFString) -> Result<Option<String>, Failure> {
    value(information, key)
        .map(|value| {
            value.downcast_ref::<CFString>().map(CFString::to_string).ok_or_else(invalid_metadata)
        })
        .transpose()
}

fn value<'a>(information: &'a CFDictionary, key: &CFString) -> Option<&'a CFType> {
    // SAFETY: the key is a CFString, which is what this dictionary is keyed by.
    let value = unsafe { information.value((key as *const CFString).cast::<c_void>()) };
    let value = NonNull::new(value.cast_mut())?;
    // SAFETY: the dictionary owns the value and outlives the borrow.
    Some(unsafe { value.cast::<CFType>().as_ref() })
}

fn invalid_metadata() -> Failure {
    Failure::internal(
        "Security.framework could not provide required signing metadata for this recon",
    )
}
