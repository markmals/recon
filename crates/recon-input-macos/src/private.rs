use std::ffi::{CStr, c_char, c_void};

use recon_core::error::{ErrorCode, Failure};
use serde_json::json;

pub(crate) fn available(symbol: &CStr) -> bool {
    // SAFETY: looking up a symbol neither calls it nor changes state.
    !unsafe { dlsym(RTLD_DEFAULT, symbol.as_ptr()) }.is_null()
}

pub(crate) unsafe fn function<T: Copy>(symbol: &CStr, interface: &str) -> Result<T, Failure> {
    // SAFETY: dlsym returns the address of the requested C symbol or null; transmute_copy preserves
    // that pointer's bits in the function-pointer type the caller declares.
    let pointer = unsafe { dlsym(RTLD_DEFAULT, symbol.as_ptr()) };
    if pointer.is_null() {
        return Err(Failure::new(
            ErrorCode::InterfaceUnavailable,
            format!("{interface} is unavailable: {}", symbol.to_string_lossy()),
            json!({"interface":interface,"symbol":symbol.to_string_lossy()}),
        ));
    }
    // SAFETY: the caller binds a private symbol to its documented C signature, and T is a function
    // pointer with the same representation as the data pointer returned by dlsym.
    Ok(unsafe { std::mem::transmute_copy(&pointer) })
}

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

const RTLD_DEFAULT: *mut c_void = -2_isize as *mut c_void;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal0007_a_missing_private_symbol_is_interface_unavailable() {
        type Missing = unsafe extern "C" fn();
        // SAFETY: the deliberately absent symbol is never called; only dlsym's null result is used.
        let failure = unsafe {
            function::<Missing>(c"ReconDefinitelyMissingInteractionSymbol", "test-interface")
        }
        .unwrap_err();
        assert_eq!(failure.code, ErrorCode::InterfaceUnavailable);
        assert_eq!(
            failure.details.expect("details")["symbol"],
            "ReconDefinitelyMissingInteractionSymbol"
        );
    }
}
