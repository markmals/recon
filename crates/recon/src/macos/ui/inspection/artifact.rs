//! Locating the matching packaged helper/loader and starting the helper in a bound process.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::inspection::{AccessProfile, Incarnation, InspectionError, InspectionErrorCode, Stage};

pub(super) struct Artifacts {
    pub helper: PathBuf,
    pub helper_build: String,
    pub loader: PathBuf,
}

pub(super) fn locate(profile: AccessProfile) -> Result<Artifacts, InspectionError> {
    let executable = std::env::current_exe().map_err(|error| {
        InspectionError::from_io(
            InspectionErrorCode::Unavailable,
            Stage::Attach,
            "the Recon executable path is unavailable",
            &error,
        )
    })?;
    let directory = executable.parent().unwrap_or_else(|| Path::new("."));
    let mut directories = vec![directory.to_path_buf()];
    if profile == AccessProfile::Research {
        directories.insert(0, PathBuf::from("/Library/Application Support/Recon"));
    }
    if matches!(
        directory.file_name().and_then(|name| name.to_str()),
        Some("debug" | "release" | "signed")
    ) && let Some(target) = directory.parent()
    {
        directories.push(target.to_path_buf());
    }

    for directory in directories {
        let helper = directory.join("recon-helper.dylib");
        let loader = directory.join("recon-loader");
        let identity = directory.join("recon-helper.build-id");
        if helper.is_file() && loader.is_file() && identity.is_file() {
            let helper_build = fs::read_to_string(&identity)
                .map_err(|error| {
                    InspectionError::from_io(
                        InspectionErrorCode::Unavailable,
                        Stage::Attach,
                        "the helper build identity could not be read",
                        &error,
                    )
                })?
                .trim()
                .to_owned();
            if helper_build.len() == 64 && helper_build.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Ok(Artifacts { helper, helper_build, loader });
            }
        }
    }
    Err(InspectionError::new(
        InspectionErrorCode::Unavailable,
        Stage::Attach,
        "matching recon-helper.dylib, recon-helper.build-id, and recon-loader artifacts are not installed",
    ))
}

pub(super) fn load(
    artifacts: &Artifacts,
    incarnation: Incarnation,
    socket: &Path,
    expected_executable: &Path,
    deadline: Instant,
) -> Result<(), InspectionError> {
    if Instant::now() >= deadline {
        return Err(InspectionError::new(
            InspectionErrorCode::Timeout,
            Stage::Attach,
            "the open deadline expired before the Recon loader started",
        ));
    }
    let mut child = Command::new(&artifacts.loader)
        .arg(incarnation.pid.to_string())
        .arg(incarnation.pidversion.to_string())
        .arg(&artifacts.helper)
        .arg(socket)
        .arg(expected_executable)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            InspectionError::from_io(
                InspectionErrorCode::Unavailable,
                Stage::Attach,
                "the Recon loader could not be started",
                &error,
            )
        })?;

    loop {
        if let Some(status) = child.try_wait().map_err(|error| {
            InspectionError::from_io(
                InspectionErrorCode::Failed,
                Stage::Attach,
                "the Recon loader status could not be read",
                &error,
            )
        })? {
            if status.success() {
                return Ok(());
            }
            let output = child.wait_with_output().map_err(|error| {
                InspectionError::from_io(
                    InspectionErrorCode::Failed,
                    Stage::Attach,
                    "the Recon loader result could not be read",
                    &error,
                )
            })?;
            return Err(InspectionError::new(
                InspectionErrorCode::Unavailable,
                Stage::Attach,
                format!(
                    "the Recon loader refused the selected incarnation: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ),
            ));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(InspectionError::new(
                InspectionErrorCode::Timeout,
                Stage::Attach,
                "the Recon loader exceeded the ten-second open deadline",
            ));
        }
        sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn loader_uses_the_remaining_open_budget() {
        let temporary = tempfile::tempdir().expect("private loader fixture");
        let loader = temporary.path().join("slow-loader");
        fs::write(&loader, "#!/bin/sh\nsleep 1\n").expect("write the fake loader");
        fs::set_permissions(&loader, fs::Permissions::from_mode(0o700))
            .expect("make the fake loader executable");
        let artifacts = Artifacts {
            loader,
            helper: temporary.path().join("helper.dylib"),
            helper_build: "fixture-build".to_owned(),
        };
        let started = Instant::now();
        let failure = load(
            &artifacts,
            Incarnation { pid: 42, pidversion: 7 },
            &temporary.path().join("socket"),
            Path::new("/bin/true"),
            started + Duration::from_millis(40),
        )
        .expect_err("the loader cannot take a fresh ten seconds");
        assert_eq!(failure.code(), InspectionErrorCode::Timeout);
        assert!(started.elapsed() < Duration::from_millis(500), "{:?}", started.elapsed());
    }
}
