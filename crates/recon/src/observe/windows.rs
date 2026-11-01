//! `recon ui windows`: the window server's inventory, and with a session the target's own
//! native window inventory beside it. The two stay separate evidence.

use recon_applications_macos::selection::Selection;
use recon_core::action::{Invocation, invoke};
use recon_core::error::Failure;
use recon_native_access_macos::SessionId;
use recon_native_access_macos::appkit::{AppKit, WINDOWS};
use recon_privacy_macos::permission::Permission;
use recon_privacy_macos::permissions;
use serde_json::Map;

use super::bound;
use super::target::{observed, with_instance};
use crate::ui::response::WindowsResponse;

pub(super) fn windows(
    selection: &Selection,
    session: Option<&SessionId>,
) -> Result<String, Failure> {
    with_instance(selection, session, |application, pid| {
        let summary = bound(application, pid, session)?;
        permissions::require(&[Permission::ScreenRecording])?;
        let windows = recon_window_server_macos::inventory::windows_owned_by(pid)?;
        let mut response =
            serde_json::to_value(WindowsResponse::new(observed(application, pid), windows))
                .map_err(|error| Failure::internal(error.to_string()))?;
        if let (Some(session), Some(summary)) = (session, summary) {
            let extension = AppKit::new(application, pid, session);
            let native = invoke(&extension, Invocation { action: WINDOWS, input: Map::new() })?;
            response["session"] = serde_json::to_value(summary)
                .map_err(|error| Failure::internal(error.to_string()))?;
            response["nativeWindows"] = native["windows"].clone();
            response["coverage"] = native["coverage"].clone();
            response["observation"] = native["observation"].clone();
        }
        crate::output::encode(&response)
    })
}
