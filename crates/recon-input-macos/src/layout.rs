use std::ffi::c_void;

use recon_core::error::{
    ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};

use crate::chord::{BaseKey, Chord};
use crate::failure::action;
use crate::target::Target;

pub fn keycode(target: &Target, chord: &Chord) -> Result<u16, Failure> {
    match chord.key {
        BaseKey::Named(name) => Ok(named_keycode(name)),
        BaseKey::Character(character) => character_keycode(character).ok_or_else(|| {
            action(
                ErrorCode::ActionUnavailable,
                format!("the current keyboard layout has no unmodified key for {character}"),
                crate::failure::details(
                    target,
                    ActionMechanism::KeyEvents,
                    ActionStage::Resolve,
                    DispatchState::None,
                    ReleaseState::NotNeeded,
                ),
            )
        }),
    }
}

fn named_keycode(name: &str) -> u16 {
    match name {
        "return" => 36,
        "tab" => 48,
        "space" => 49,
        "escape" => 53,
        "delete" => 51,
        "forward-delete" => 117,
        "left" => 123,
        "right" => 124,
        "up" => 126,
        "down" => 125,
        "home" => 115,
        "end" => 119,
        "pageup" => 116,
        "pagedown" => 121,
        "f1" => 122,
        "f2" => 120,
        "f3" => 99,
        "f4" => 118,
        "f5" => 96,
        "f6" => 97,
        "f7" => 98,
        "f8" => 100,
        "f9" => 101,
        "f10" => 109,
        "f11" => 103,
        "f12" => 111,
        _ => unreachable!("parsed named key"),
    }
}

fn character_keycode(character: char) -> Option<u16> {
    // SAFETY: the input-source and CFData calls follow Carbon's ownership rules. The returned
    // layout data belongs to the input source, which remains alive for the whole loop.
    unsafe {
        let source = TISCopyCurrentKeyboardLayoutInputSource();
        if source.is_null() {
            return None;
        }
        let data = TISGetInputSourceProperty(source, TISPropertyUnicodeKeyLayoutData);
        if data.is_null() {
            CFRelease(source);
            return None;
        }
        let bytes = CFDataGetBytePtr(data);
        if bytes.is_null() {
            CFRelease(source);
            return None;
        }
        let keyboard_type = LMGetKbdType();
        let mut found = None;
        for keycode in 0..=127_u16 {
            let mut dead_state = 0_u32;
            let mut actual = 0_usize;
            let mut translated = [0_u16; 2];
            let status = UCKeyTranslate(
                bytes.cast(),
                keycode,
                0,
                0,
                keyboard_type,
                1,
                &mut dead_state,
                translated.len(),
                &mut actual,
                translated.as_mut_ptr(),
            );
            if status == 0 && actual == 1 && char::from_u32(translated[0] as u32) == Some(character)
            {
                found = Some(keycode);
                break;
            }
        }
        CFRelease(source);
        found
    }
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    #[link_name = "kTISPropertyUnicodeKeyLayoutData"]
    static TISPropertyUnicodeKeyLayoutData: *const c_void;
    fn TISCopyCurrentKeyboardLayoutInputSource() -> *mut c_void;
    fn TISGetInputSourceProperty(source: *mut c_void, property: *const c_void) -> *mut c_void;
    fn LMGetKbdType() -> u32;
    fn UCKeyTranslate(
        layout: *const c_void,
        keycode: u16,
        action: u16,
        modifiers: u32,
        keyboard_type: u32,
        options: u32,
        dead_state: *mut u32,
        maximum: usize,
        actual: *mut usize,
        translated: *mut u16,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDataGetBytePtr(data: *mut c_void) -> *const u8;
    fn CFRelease(object: *mut c_void);
}
