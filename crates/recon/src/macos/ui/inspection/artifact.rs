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
) -> Result<(), InspectionError> {
    let mut child = Command::new(&artifacts.loader)
        .arg(incarnation.pid.to_string())
        .arg(incarnation.pidversion.to_string())
        .arg(&artifacts.helper)
        .arg(socket)
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

    let deadline = Instant::now() + Duration::from_secs(10);
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
