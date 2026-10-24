//! Choosing which running instance of a bundle `recon ui` observes.

use std::path::Path;

use crate::application::Pid;

/// Picks the one running process of a resolved bundle, never guessing between several.
///
/// # Errors
///
/// When the bundle is not running, when `requested` is not one of its processes, or when it has
/// several and none was requested.
pub fn select_instance(
    application_path: &Path,
    running_pids: &[Pid],
    requested: Option<Pid>,
) -> Result<Pid, String> {
    let application = application_path.display();
    if running_pids.is_empty() {
        return Err(format!("{application} is not running; recon ui does not launch applications"));
    }
    if let Some(pid) = requested {
        if running_pids.contains(&pid) {
            return Ok(pid);
        }
        return Err(format!("process {pid} is not a running instance of {application}"));
    }
    if let [only] = running_pids {
        return Ok(*only);
    }
    let candidates = running_pids.iter().map(Pid::to_string).collect::<Vec<_>>().join(", ");
    Err(format!("{application} has several running instances; choose one with --pid: {candidates}"))
}
