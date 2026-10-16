use std::path::Path;

use objc2_foundation::{NSFileManager, NSString};
use plist::Value;

use crate::application::Application;

pub fn inspect_bundle(path: &Path) -> Result<Application, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("cannot access {}: {error}", path.display()))?;
    // Like macOS, recognize an application bundle by its `app` extension in any case, not by its
    // `CFBundlePackageType`: Finder declares `FNDR`, and Passwords declares `XPC!`.
    if !path.is_dir()
        || path.extension().is_none_or(|extension| !extension.eq_ignore_ascii_case("app"))
    {
        return Err(format!("not an application: {}", path.display()));
    }

    let info_path = path.join("Contents/Info.plist");
    let info = Value::from_file(&info_path)
        .map_err(|error| format!("not an application: {} ({error})", path.display()))?;
    let info =
        info.as_dictionary().ok_or_else(|| format!("not an application: {}", path.display()))?;
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
        .ok_or_else(|| format!("application has no name: {}", path.display()))?;
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
