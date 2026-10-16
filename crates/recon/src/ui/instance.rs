use std::path::Path;

/// Picks the one running process of a resolved bundle, never guessing between several.
pub fn select_instance(
    application_path: &Path,
    running_pids: &[u32],
    requested: Option<u32>,
) -> Result<u32, String> {
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
    let candidates = running_pids.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
    Err(format!("{application} has several running instances; choose one with --pid: {candidates}"))
}
