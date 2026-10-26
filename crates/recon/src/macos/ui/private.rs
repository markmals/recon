use std::ffi::{CStr, c_void};
use std::mem::{size_of, transmute_copy};

/// Resolves a private macOS function at runtime.
///
/// Recon links nothing private, so an interface that a macOS update removes becomes a reported
/// unavailable capability instead of a process that cannot start.
///
/// # Safety
///
/// `F` must be an `unsafe extern "C" fn` type whose signature matches the function that `name`
/// names. A safe function type would let safe code call the symbol with a signature nothing has
/// checked.
pub(super) unsafe fn function<F: Copy>(name: &CStr) -> Option<F> {
    const { assert!(size_of::<F>() == size_of::<*mut c_void>(), "not a function pointer") };
    // SAFETY: `RTLD_DEFAULT` searches the already-loaded images for a null-terminated name.
    let address = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    if address.is_null() {
        return None;
    }
    // SAFETY: `F` is pointer sized, and the caller guarantees it is the function's signature.
    Some(unsafe { transmute_copy(&address) })
}
