mod metadata;
mod workspace;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::{Command, Output};

use crate::application::{Application, TargetError, normalize_applications, resolve_target};

pub use workspace::launch;

pub fn applications() -> Result<Vec<Application>, String> {
    let output = Command::new("/usr/bin/mdfind")
        .args(["-0", "kMDItemContentTypeTree == 'com.apple.application-bundle'"])
        .output()
        .map_err(|error| format!("cannot search indexed applications: {error}"))?;
    let indexed_paths = checked_indexed_output(&output)?;
    let mut discovered: Vec<Application> = indexed_paths
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .filter_map(|path| metadata::inspect_bundle(Path::new(OsStr::from_bytes(path))).ok())
        .collect();
    for (path, pid) in workspace::running_applications() {
        if let Some(application) = discovered.iter_mut().find(|app| app.path == path) {
            application.running_pids.push(pid);
        } else if let Ok(mut application) = metadata::inspect_bundle(&path) {
            application.running_pids.push(pid);
            discovered.push(application);
        }
    }
    Ok(normalize_applications(discovered))
}

fn checked_indexed_output(output: &Output) -> Result<&[u8], String> {
    if !output.status.success() {
        return Err(format!(
            "cannot search indexed applications: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if output.stdout.iter().all(|byte| *byte == 0) {
        return Err("cannot search indexed applications: no indexed applications found; check Spotlight indexing".to_owned());
    }
    Ok(&output.stdout)
}

pub fn with_target<T>(
    selector: &str,
    use_application: impl FnOnce(&Application) -> Result<T, String>,
) -> Result<T, String> {
    if Path::new(selector).is_absolute() {
        let mut application = metadata::inspect_bundle(Path::new(selector))?;
        application.running_pids = workspace::running_applications()
            .into_iter()
            .filter_map(|(path, pid)| (path == application.path).then_some(pid))
            .collect();
        application.running_pids.sort_unstable();
        application.running_pids.dedup();
        return use_application(&application);
    }

    let applications = applications()?;
    let application = resolve_target(&applications, selector).map_err(|error| match error {
        TargetError::NotFound => format!("no application matches {selector:?}"),
        TargetError::Ambiguous(candidates) => format!(
            "ambiguous application {selector:?}; choose a bundle path:\n{}",
            candidates
                .iter()
                .map(|app| format!(
                    "  {} [{}] {}",
                    app.name,
                    app.bundle_id.as_deref().unwrap_or("-"),
                    app.path.display()
                ))
                .collect::<Vec<_>>()
                .join("\n")
        ),
    })?;
    use_application(application)
}

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;
    use std::process::{ExitStatus, Output};

    use super::checked_indexed_output;

    #[test]
    fn an_empty_index_does_not_claim_indexed_discovery() {
        let output =
            Output { status: ExitStatus::from_raw(0), stdout: Vec::new(), stderr: Vec::new() };
        assert!(checked_indexed_output(&output).unwrap_err().contains("Spotlight indexing"));
    }

    #[test]
    fn a_failed_index_query_reports_its_failure() {
        let output = Output {
            status: ExitStatus::from_raw(1 << 8),
            stdout: Vec::new(),
            stderr: b"metadata server unavailable".to_vec(),
        };
        let error = checked_indexed_output(&output).unwrap_err();
        assert!(error.contains("metadata server unavailable"));
        assert!(error.contains("cannot search indexed applications"));
    }
}
