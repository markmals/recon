use std::ffi::{CStr, c_void};
use std::mem::{size_of, transmute_copy};

/// Resolves a private macOS function at runtime.
///
/// Recon links nothing private, so removed symbols are handled at runtime rather than
/// preventing startup.
///
/// # Safety
///
/// `F` must be an `unsafe extern "C" fn` type whose signature matches the function that `name`
/// names. A safe function type would let safe code call the symbol with a signature nothing has
/// checked.
pub(super) unsafe fn function<F: Copy>(name: &CStr) -> Option<F> {
    const { assert!(size_of::<F>() == size_of::<*mut c_void>(), "not a function pointer") };
    let address = address(name)?;
    // SAFETY: `F` is pointer sized, and the caller guarantees it is the function's signature.
    Some(unsafe { transmute_copy(&address) })
}

/// Whether this macOS has the function that `name` names.
///
/// Looking a symbol up neither calls it nor needs its signature, so reporting what Recon can
/// reach costs nothing and affects nothing.
pub(super) fn available(name: &CStr) -> bool {
    address(name).is_some()
}

fn address(name: &CStr) -> Option<*mut c_void> {
    // SAFETY: `RTLD_DEFAULT` searches the already-loaded images for a null-terminated name.
    let address = unsafe { libc::dlsym(libc::RTLD_DEFAULT, name.as_ptr()) };
    (!address.is_null()).then_some(address)
}
