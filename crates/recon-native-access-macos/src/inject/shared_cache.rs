//! Confirming a target shares this process's dyld shared cache.
//!
//! When two processes map the same shared cache at the same address, every shared-cache symbol
//! sits at the same address in both. The loader relies on that to resolve the libSystem entry
//! points it hands the target (`pthread_create_from_mach_thread`, `dlopen`, `dlsym`) from its own
//! address space. A mismatch means those addresses would be wrong, so injection is unavailable.

use libc::mach_port_t;

use super::LoaderError;
use super::mach::{self, DyldProcessCacheInfo, KERN_SUCCESS};

/// A dyld shared cache's identity: its UUID and the address it is mapped at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SharedCache {
    pub uuid: [u8; 16],
    pub base: usize,
}

/// This process's shared cache, if it has one.
fn own() -> Option<SharedCache> {
    let mut uuid = [0u8; 16];
    // SAFETY: `uuid` is exactly a 16-byte uuid_t buffer.
    let present = unsafe { mach::_dyld_get_shared_cache_uuid(&mut uuid) };
    if !present {
        return None;
    }
    let mut length = 0usize;
    // SAFETY: `length` is a valid out pointer; the return is the cache's base address.
    let base = unsafe { mach::_dyld_get_shared_cache_range(&mut length) };
    if base.is_null() {
        return None;
    }
    Some(SharedCache { uuid, base: base as usize })
}

/// Confirm `task` shares this process's shared cache at the same address. Reads only; nothing is
/// written to the target.
pub fn verify(task: mach_port_t) -> Result<SharedCache, LoaderError> {
    let ours = own().ok_or(LoaderError::SharedCacheMismatch)?;
    let theirs = target_cache(task)?;
    if theirs.uuid == ours.uuid && theirs.base == ours.base {
        Ok(ours)
    } else {
        Err(LoaderError::SharedCacheMismatch)
    }
}

fn target_cache(task: mach_port_t) -> Result<SharedCache, LoaderError> {
    let mut kr = KERN_SUCCESS;
    // SAFETY: `task` is a live control port; `kr` is a valid out pointer.
    let info = unsafe { mach::_dyld_process_info_create(task, 0, &mut kr) };
    if info.is_null() || kr != KERN_SUCCESS {
        if !info.is_null() {
            // SAFETY: release a handle that was created despite the reported error.
            unsafe { mach::_dyld_process_info_release(info) };
        }
        return Err(LoaderError::DyldInfoUnavailable(kr));
    }
    let mut cache = DyldProcessCacheInfo {
        cache_uuid: [0; 16],
        cache_base_address: 0,
        no_cache: false,
        private_cache: false,
    };
    // SAFETY: `info` is live and `cache` is a valid out struct of the expected layout.
    unsafe { mach::_dyld_process_info_get_cache(info, &mut cache) };
    // SAFETY: release the handle created above exactly once.
    unsafe { mach::_dyld_process_info_release(info) };
    if cache.no_cache {
        return Err(LoaderError::SharedCacheMismatch);
    }
    Ok(SharedCache { uuid: cache.cache_uuid, base: cache.cache_base_address as usize })
}
