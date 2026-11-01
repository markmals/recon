//! The Mach traps and dyld queries the loader needs, declared directly because `libc` does not
//! expose all of them (and deprecates the ones it does). These are thin FFI bindings; the safe
//! wrappers live beside them.

use std::ffi::c_void;

use libc::{c_int, c_uint, kern_return_t, mach_msg_type_number_t, mach_port_t, pid_t};

pub(crate) const KERN_SUCCESS: kern_return_t = 0;

/// `task_info` flavor returning the target's BSM audit token.
pub(crate) const TASK_AUDIT_TOKEN: c_int = 15;
/// The audit token is eight `natural_t` words long.
pub(crate) const TASK_AUDIT_TOKEN_COUNT: mach_msg_type_number_t = 8;

/// A process's audit token. `val[5]` is the pid; `val[7]` is the pid generation that distinguishes
/// one incarnation from a later process reusing the same number.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct AuditToken {
    pub(crate) val: [c_uint; 8],
}

/// Mirror of dyld's `dyld_process_cache_info` from `<mach-o/dyld_process_info.h>`.
#[repr(C)]
pub(crate) struct DyldProcessCacheInfo {
    pub(crate) cache_uuid: [u8; 16],
    pub(crate) cache_base_address: u64,
    pub(crate) no_cache: bool,
    pub(crate) private_cache: bool,
}

/// This process's own task control port. `mach_task_self_` is a global the runtime initializes
/// before `main`, so reading it is a plain load.
pub(crate) fn mach_task_self() -> mach_port_t {
    // SAFETY: `mach_task_self_` is an immutable global set up by libSystem before `main`.
    unsafe { mach_task_self_ }
}

unsafe extern "C" {
    static mach_task_self_: mach_port_t;

    pub(crate) fn task_for_pid(
        target: mach_port_t,
        pid: pid_t,
        task: *mut mach_port_t,
    ) -> kern_return_t;
    pub(crate) fn task_info(
        task: mach_port_t,
        flavor: c_int,
        info: *mut c_int,
        count: *mut mach_msg_type_number_t,
    ) -> kern_return_t;
    pub(crate) fn mach_port_deallocate(task: mach_port_t, name: mach_port_t) -> kern_return_t;

    pub(crate) fn _dyld_get_shared_cache_uuid(uuid: *mut [u8; 16]) -> bool;
    pub(crate) fn _dyld_get_shared_cache_range(length: *mut usize) -> *const c_void;
    pub(crate) fn _dyld_process_info_create(
        task: mach_port_t,
        timestamp: u64,
        kr: *mut kern_return_t,
    ) -> *mut c_void;
    pub(crate) fn _dyld_process_info_get_cache(
        info: *mut c_void,
        cache_info: *mut DyldProcessCacheInfo,
    );
    pub(crate) fn _dyld_process_info_release(info: *mut c_void);
}
