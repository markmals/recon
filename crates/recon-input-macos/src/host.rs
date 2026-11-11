use std::ffi::c_void;

use objc2_core_foundation::CFString;

use recon_core::error::{
    ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};

use crate::failure::action;
use crate::target::Target;

pub fn require_unlocked(target: &Target, mechanism: ActionMechanism) -> Result<(), Failure> {
    // SAFETY: the session dictionary is copied and released; its boolean values are borrowed only
    // for this check.
    unsafe {
        let session = CGSessionCopyCurrentDictionary();
        if session.is_null() {
            return Err(unavailable(
                target,
                mechanism,
                "the current console session is unavailable",
            ));
        }
        let on_console = CFDictionaryGetValue(session, CGSessionOnConsoleKey);
        let locked_key = CFString::from_str("CGSSessionScreenIsLocked");
        let locked = CFDictionaryGetValue(session, (&*locked_key as *const CFString).cast());
        let allowed = !on_console.is_null()
            && CFBooleanGetValue(on_console)
            && (locked.is_null() || !CFBooleanGetValue(locked));
        CFRelease(session);
        if allowed {
            Ok(())
        } else {
            Err(unavailable(
                target,
                mechanism,
                "synthetic input requires an unlocked console session",
            ))
        }
    }
}

pub fn require_secure_input_disabled(target: &Target) -> Result<(), Failure> {
    // SAFETY: IsSecureEventInputEnabled reads this session's secure-input state.
    if unsafe { IsSecureEventInputEnabled() } {
        Err(unavailable(target, ActionMechanism::KeyEvents, "secure event input is enabled"))
    } else {
        Ok(())
    }
}

fn unavailable(target: &Target, mechanism: ActionMechanism, reason: &str) -> Failure {
    action(
        ErrorCode::ActionUnavailable,
        reason,
        crate::failure::details(
            target,
            mechanism,
            ActionStage::Resolve,
            DispatchState::None,
            ReleaseState::NotNeeded,
        ),
    )
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    #[link_name = "kCGSSessionOnConsoleKey"]
    static CGSessionOnConsoleKey: *const c_void;
    fn CGSessionCopyCurrentDictionary() -> *mut c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDictionaryGetValue(dictionary: *const c_void, key: *const c_void) -> *const c_void;
    fn CFBooleanGetValue(value: *const c_void) -> bool;
    fn CFRelease(object: *mut c_void);
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn IsSecureEventInputEnabled() -> bool;
}
