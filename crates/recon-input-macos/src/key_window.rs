use std::ffi::CStr;
use std::thread::sleep;
use std::time::Duration;

use recon_core::error::{
    ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};
use recon_core::process::Pid;
use recon_window_server_macos::window::WindowId;

use crate::failure::action;
use crate::target::Target;

#[repr(C)]
struct ProcessSerialNumber {
    high: u32,
    low: u32,
}

pub fn frontmost_pid() -> Result<Option<Pid>, Failure> {
    unsafe {
        // SAFETY: these private SkyLight calls read the front process without changing it.
        let front: unsafe extern "C" fn(*mut ProcessSerialNumber) -> i32 =
            symbol(c"_SLPSGetFrontProcess", "front-process")?;
        let pid_of: unsafe extern "C" fn(*const ProcessSerialNumber, *mut i32) -> i32 =
            symbol(c"GetProcessPID", "front-process-pid")?;
        let mut psn = ProcessSerialNumber { high: 0, low: 0 };
        if front(&mut psn) != 0 {
            return Ok(None);
        }
        let mut pid = 0;
        if pid_of(&psn, &mut pid) != 0 {
            return Ok(None);
        }
        Ok(u32::try_from(pid).ok().map(Pid))
    }
}

pub fn make_appkit_key(
    target: &Target,
    previous: Option<WindowId>,
    mut interrupted: impl FnMut() -> bool,
) -> Result<(), Failure> {
    if frontmost_pid()? == Some(target.pid()) {
        return Err(key_window_unavailable(
            target,
            "a frontmost target's key window cannot be changed",
        ));
    }
    unsafe {
        // SAFETY: the private functions are bound to their SkyLight C signatures and every event
        // record is addressed only to the target process's PSN.
        let connection: unsafe extern "C" fn() -> u32 =
            symbol(c"CGSMainConnectionID", "window-server-connection")?;
        let owner_of: unsafe extern "C" fn(u32, u32, *mut u32) -> i32 =
            symbol(c"SLSGetWindowOwner", "window-owner")?;
        let psn_of: unsafe extern "C" fn(u32, *mut ProcessSerialNumber) -> i32 =
            symbol(c"SLSGetConnectionPSN", "window-owner-psn")?;
        let post: unsafe extern "C" fn(*const ProcessSerialNumber, *const u8) -> i32 =
            symbol(c"SLPSPostEventRecordTo", "target-key-window-records")?;
        let mut owner = 0;
        if owner_of(connection(), target.window().id.0, &mut owner) != 0 {
            return Err(key_window_failure(
                target,
                "the selected window's WindowServer owner could not be read",
            ));
        }
        let mut psn = ProcessSerialNumber { high: 0, low: 0 };
        if psn_of(owner, &mut psn) != 0 {
            return Err(key_window_failure(
                target,
                "the selected window owner's process serial number could not be read",
            ));
        }
        for stage in 0..4 {
            if previous.is_none() && stage == 0 {
                continue;
            }
            if interrupted() {
                return Err(key_window_failure(
                    target,
                    "the interaction guard stopped the remaining key-window records",
                ));
            }
            let dispatch = if stage == 0 || (stage == 1 && previous.is_none()) {
                DispatchState::None
            } else {
                DispatchState::Partial
            };
            target.revalidate(ActionMechanism::KeyEvents, dispatch, ReleaseState::NotNeeded)?;
            let mut record = [0_u8; 248];
            record[4] = 248;
            let addressed = if stage == 0 {
                previous.expect("defocus has a previous window")
            } else {
                target.window().id
            };
            record[0x3c..0x40].copy_from_slice(&addressed.0.to_ne_bytes());
            if stage < 2 {
                record[8] = 13;
                record[0x8a] = if stage == 0 { 2 } else { 1 };
            } else {
                record[8] = if stage == 2 { 1 } else { 2 };
                record[0x3a] = 0x10;
                record[0x20..0x30].fill(0xff);
            }
            if post(&psn, record.as_ptr()) != 0 {
                return Err(key_window_failure(
                    target,
                    "WindowServer rejected a target-only key-window record",
                ));
            }
            if stage < 2 {
                sleep(Duration::from_millis(120));
            }
        }
    }
    Ok(())
}

fn key_window_unavailable(target: &Target, detail: &str) -> Failure {
    action(
        ErrorCode::ActionUnavailable,
        detail,
        crate::failure::details(
            target,
            ActionMechanism::KeyEvents,
            ActionStage::KeyWindow,
            DispatchState::None,
            ReleaseState::NotNeeded,
        ),
    )
}

fn key_window_failure(target: &Target, detail: &str) -> Failure {
    action(
        ErrorCode::ActionFailed,
        detail,
        crate::failure::details(
            target,
            ActionMechanism::KeyEvents,
            ActionStage::KeyWindow,
            DispatchState::None,
            ReleaseState::NotNeeded,
        ),
    )
}

unsafe fn symbol<T: Copy>(name: &CStr, interface: &str) -> Result<T, Failure> {
    // SAFETY: the caller supplies the private symbol's C function signature.
    unsafe { crate::private::function(name, interface) }
}
