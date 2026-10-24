use std::path::{Path, PathBuf};
use std::sync::mpsc;

use block2::RcBlock;
use objc2_app_kit::{NSRunningApplication, NSWorkspace, NSWorkspaceOpenConfiguration};
use objc2_foundation::{NSError, NSString, NSURL};

use crate::application::Pid;

pub(super) fn running_applications() -> Vec<(PathBuf, Pid)> {
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter_map(|running| {
            let pid = Pid(u32::try_from(running.processIdentifier()).ok()?);
            let path = PathBuf::from(running.bundleURL()?.path()?.to_string());
            let path = path.canonicalize().ok()?;
            Some((path, pid))
        })
        .collect()
}

pub(super) fn launch(path: &Path) -> Result<Pid, String> {
    let path_string = path
        .to_str()
        .ok_or_else(|| format!("application path is not UTF-8: {}", path.display()))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path_string));
    let configuration = NSWorkspaceOpenConfiguration::configuration();
    configuration.setActivates(true);
    configuration.setCreatesNewApplicationInstance(false);
    configuration.setAllowsRunningApplicationSubstitution(false);

    let (sender, receiver) = mpsc::sync_channel(1);
    let completion =
        RcBlock::new(move |running: *mut NSRunningApplication, error: *mut NSError| {
            // SAFETY: AppKit passes each argument as null or as an object that stays valid for
            // the duration of the completion handler.
            let response = unsafe {
                if let Some(error) = error.as_ref() {
                    Err(error.localizedDescription().to_string())
                } else if let Some(running) = running.as_ref() {
                    u32::try_from(running.processIdentifier())
                        .map(Pid)
                        .map_err(|_| "macOS opened an application without a process ID".to_owned())
                } else {
                    Err("macOS did not open the application".to_owned())
                }
            };
            // The receiver is gone only after `launch` has returned, when no one is left to tell.
            let _ = sender.send(response);
        });
    NSWorkspace::sharedWorkspace().openApplicationAtURL_configuration_completionHandler(
        &url,
        &configuration,
        Some(&completion),
    );
    receiver
        .recv()
        .map_err(|error| format!("macOS launch callback failed: {error}"))?
        .map_err(|error| format!("cannot launch {}: {error}", path.display()))
}
