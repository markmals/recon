use std::path::{Path, PathBuf};
use std::sync::mpsc;

use block2::RcBlock;
use objc2_app_kit::{NSRunningApplication, NSWorkspace, NSWorkspaceOpenConfiguration};
use objc2_foundation::{NSError, NSString, NSURL};

pub fn running_applications() -> Vec<(PathBuf, u32)> {
    NSWorkspace::sharedWorkspace()
        .runningApplications()
        .iter()
        .filter_map(|running| {
            let pid = u32::try_from(running.processIdentifier()).ok()?;
            let path = PathBuf::from(running.bundleURL()?.path()?.to_string());
            let path = path.canonicalize().ok()?;
            Some((path, pid))
        })
        .collect()
}

pub fn launch(path: &Path) -> Result<u32, String> {
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
            let response = unsafe {
                if let Some(error) = error.as_ref() {
                    Err(error.localizedDescription().to_string())
                } else if let Some(running) = running.as_ref() {
                    u32::try_from(running.processIdentifier())
                        .map_err(|_| "macOS opened an application without a process ID".to_owned())
                } else {
                    Err("macOS did not open the application".to_owned())
                }
            };
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
