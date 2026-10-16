use std::ffi::CStr;
use std::mem::{size_of, transmute_copy};

/// Resolves a private macOS function at runtime.
///
/// Recon links nothing private, so an interface that a macOS update removes becomes a reported
/// unavailable capability instead of a process that cannot start.
pub fn function<F: Copy>(name: &CStr) -> Option<F> {
    assert_eq!(size_of::<F>(), size_of::<*mut std::ffi::c_void>(), "not a function pointer");
    // SAFETY: `RTLD_DEFAULT` searches the already-loaded images for a null-terminated name.
    let address = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    if address.is_null() {
        return None;
    }
    // SAFETY: the address names the function the caller asked for, and `F` is pointer sized.
    Some(unsafe { transmute_copy(&address) })
}
