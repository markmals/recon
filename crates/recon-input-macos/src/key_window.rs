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
    // SAFETY: SLPSPostEventRecordTo is bound to its SkyLight C signature.
    let post: unsafe extern "C" fn(*const ProcessSerialNumber, *const u8) -> i32 =
        unsafe { symbol(c"SLPSPostEventRecordTo", "target-key-window-records")? };
    let psn = owner_psn(target)?;
    post_records(
        previous,
        target.window().id,
        || records_ready(target, &mut interrupted),
        |record| {
            // SAFETY: the record is a complete event record, addressed only to the target's PSN.
            if unsafe { post(&psn, record.as_ptr()) } == 0 {
                Ok(())
            } else {
                Err(key_window_failure(
                    target,
                    "WindowServer rejected a target-only key-window record",
                ))
            }
        },
    )
}

/// The process serial number of the selected window's WindowServer owner, the only process the
/// key-window records are addressed to.
fn owner_psn(target: &Target) -> Result<ProcessSerialNumber, Failure> {
    unsafe {
        // SAFETY: the private functions are bound to their SkyLight C signatures and only read.
        let connection: unsafe extern "C" fn() -> u32 =
            symbol(c"CGSMainConnectionID", "window-server-connection")?;
        let owner_of: unsafe extern "C" fn(u32, u32, *mut u32) -> i32 =
            symbol(c"SLSGetWindowOwner", "window-owner")?;
        let psn_of: unsafe extern "C" fn(u32, *mut ProcessSerialNumber) -> i32 =
            symbol(c"SLSGetConnectionPSN", "window-owner-psn")?;
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
        Ok(psn)
    }
}

/// What must hold before each step of key-window records: the guard has seen no interference,
/// and the target is still the bound incarnation that owns the selected window.
fn records_ready(target: &Target, interrupted: &mut impl FnMut() -> bool) -> Result<(), Failure> {
    if interrupted() {
        return Err(key_window_failure(
            target,
            "the interaction guard stopped the remaining key-window records",
        ));
    }
    // No key event has been sent yet, whatever records already went out.
    target
        .revalidate(
            ActionMechanism::KeyEvents,
            ActionStage::KeyWindow,
            DispatchState::None,
            ReleaseState::NotNeeded,
        )
        .map(drop)
}

type Record = [u8; 248];

const RECORD_GAP: Duration = Duration::from_millis(120);

/// Sends the records that make `selected` key inside its application, in three steps: a defocus
/// of `previous` when there is one, a focus of `selected`, and the make-key pair. `ready` runs
/// before each step and never inside one. The pair is a single step because its first record
/// without the second leaves the change half made, and there is no release that could undo it.
fn post_records(
    previous: Option<WindowId>,
    selected: WindowId,
    mut ready: impl FnMut() -> Result<(), Failure>,
    mut post: impl FnMut(&Record) -> Result<(), Failure>,
) -> Result<(), Failure> {
    if let Some(previous) = previous {
        ready()?;
        post(&focus_record(previous, false))?;
        sleep(RECORD_GAP);
    }
    ready()?;
    post(&focus_record(selected, true))?;
    sleep(RECORD_GAP);
    ready()?;
    post(&make_key_record(selected, 1))?;
    post(&make_key_record(selected, 2))
}

fn focus_record(window: WindowId, gained: bool) -> Record {
    let mut record = addressed_record(window);
    record[8] = 13;
    record[0x8a] = if gained { 1 } else { 2 };
    record
}

fn make_key_record(window: WindowId, half: u8) -> Record {
    let mut record = addressed_record(window);
    record[8] = half;
    record[0x3a] = 0x10;
    record[0x20..0x30].fill(0xff);
    record
}

fn addressed_record(window: WindowId) -> Record {
    let mut record = [0; 248];
    record[4] = 248;
    record[0x3c..0x40].copy_from_slice(&window.0.to_ne_bytes());
    record
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

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[test]
    fn proposal0007_interference_never_splits_the_make_key_pair() {
        let (previous, selected) = (WindowId(7), WindowId(8));
        let posted = RefCell::new(Vec::new());

        // The guard reports interference as soon as the first make-key record is out.
        let result = post_records(
            Some(previous),
            selected,
            || {
                if posted.borrow().contains(&make_key_record(selected, 1)) {
                    Err(key_window_failure(&Target::exited(), "interference"))
                } else {
                    Ok(())
                }
            },
            |record| {
                posted.borrow_mut().push(*record);
                Ok(())
            },
        );

        assert!(result.is_ok(), "the pair was split: {result:?}");
        assert_eq!(
            posted.into_inner(),
            [
                focus_record(previous, false),
                focus_record(selected, true),
                make_key_record(selected, 1),
                make_key_record(selected, 2),
            ]
        );
    }

    #[test]
    fn proposal0007_a_target_gone_during_key_window_records_is_stale_at_the_key_window_stage() {
        let failure = records_ready(&Target::exited(), &mut || false).unwrap_err();

        assert_eq!(failure.code, ErrorCode::StaleTarget);
        let details = failure.details.expect("details");
        assert_eq!(details["stage"], "key-window", "{details}");
        assert_eq!(details["dispatch"], "none", "no key event had been sent: {details}");
    }
}
