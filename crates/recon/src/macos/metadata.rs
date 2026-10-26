//! Reading an application bundle's identity and versions from its `Info.plist`.

use std::path::Path;

use objc2_foundation::{NSFileManager, NSString};
use plist::Value;
use serde_json::json;

use crate::application::Application;
use crate::error::{ErrorCode, Failure, NotFound};

/// Describes the `path` bundle, an existing `.app` directory (case-insensitive) with a readable
/// `Contents/Info.plist` dictionary. Missing optional metadata is reported as absent.
///
/// # Errors
///
/// [`ErrorCode::NotFound`] when the path cannot be read, and [`ErrorCode::NotAnApplication`]
/// when it can but names no readable application bundle, with the reason it was rejected.
pub(super) fn inspect_bundle(path: &Path) -> Result<Application, Failure> {
    let path = path.canonicalize().map_err(|error| {
        Failure::new(
            ErrorCode::NotFound,
            format!("cannot access {}: {error}", path.display()),
            NotFound {
                kind: "path",
                path: Some(path.display().to_string()),
                detail: Some(error.to_string()),
                ..Default::default()
            }
            .details(),
        )
    })?;
    // Like macOS, recognize an application bundle by its `app` extension in any case, not by its
    // `CFBundlePackageType`: Finder declares `FNDR`, and Passwords declares `XPC!`.
    if !path.is_dir()
        || path.extension().is_none_or(|extension| !extension.eq_ignore_ascii_case("app"))
    {
        return Err(not_an_application(&path, "not-a-bundle-directory", None));
    }

    let info_path = path.join("Contents/Info.plist");
    let info = Value::from_file(&info_path).map_err(|error| {
        not_an_application(&path, "unreadable-info-plist", Some(error.to_string()))
    })?;
    let info = info.as_dictionary().ok_or_else(|| {
        let detail = "Contents/Info.plist is not a dictionary".to_owned();
        not_an_application(&path, "unreadable-info-plist", Some(detail))
    })?;
    let field = |key: &str| info.get(key).and_then(Value::as_string);

    let name = path
        .to_str()
        .map(|path| {
            NSFileManager::defaultManager().displayNameAtPath(&NSString::from_str(path)).to_string()
        })
        .filter(|name| !name.is_empty())
        .or_else(|| {
            field("CFBundleDisplayName").or_else(|| field("CFBundleName")).map(str::to_owned)
        })
        .or_else(|| path.file_stem().map(|name| name.to_string_lossy().into_owned()))
        .ok_or_else(|| {
            Failure::new(
                ErrorCode::NotAnApplication,
                format!("application has no name: {}", path.display()),
                json!({
                    "path": path.display().to_string(),
                    "reason": "no-name",
                    "detail": null,
                }),
            )
        })?;
    let executable_path = field("CFBundleExecutable")
        .map(|executable| path.join("Contents/MacOS").join(executable))
        .filter(|executable| executable.is_file());
    Ok(Application {
        name,
        bundle_id: field("CFBundleIdentifier").map(str::to_owned),
        path,
        version: field("CFBundleShortVersionString").map(str::to_owned),
        build: field("CFBundleVersion").map(str::to_owned),
        executable_path,
        running_pids: Vec::new(),
    })
}

/// A path that exists but is no application bundle Recon can read, with the reason it is not.
fn not_an_application(path: &Path, reason: &str, detail: Option<String>) -> Failure {
    Failure::new(
        ErrorCode::NotAnApplication,
        format!("not an application: {}", path.display()),
        json!({
            "path": path.display().to_string(),
            "reason": reason,
            "detail": detail,
        }),
    )
}
