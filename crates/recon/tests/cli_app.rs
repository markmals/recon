//! `recon app inspect` against application bundles built on disk. Inspection reads bundle metadata
//! and needs no privacy permission, so these tests are safe to run against the `cargo` build.
#![cfg(target_os = "macos")]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const BUNDLE_ID: &str = "test.recon.bundle";

fn scratch() -> TempDir {
    tempfile::tempdir().expect("create the scratch directory")
}

fn write_bundle(path: &Path, package_type: Option<&str>) {
    let contents = path.join("Contents");
    fs::create_dir_all(&contents).expect("create the bundle");
    let package_type = package_type
        .map(|package_type| {
            format!("<key>CFBundlePackageType</key><string>{package_type}</string>")
        })
        .unwrap_or_default();
    let info = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict><key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string>{package_type}</dict></plist>
"#
    );
    fs::write(contents.join("Info.plist"), info).expect("write the bundle's Info.plist");
}

fn inspect(path: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_recon"))
        .args(["app", "inspect"])
        .arg(path)
        .output()
        .expect("start recon")
}

fn inspected(path: &Path) -> Value {
    let output = inspect(path);
    assert!(output.status.success(), "{path:?}: {}", String::from_utf8_lossy(&output.stderr));
    let response: Value = serde_json::from_slice(&output.stdout).expect("JSON response");
    response["application"].clone()
}

fn rejection(path: &Path) -> String {
    let output = inspect(path);
    assert!(!output.status.success(), "{path:?} should be rejected");
    assert!(output.stdout.is_empty(), "{path:?} should print no result");
    String::from_utf8(output.stderr).expect("UTF-8 diagnostic")
}

#[test]
fn a_bundle_is_an_application_whatever_package_type_it_declares() {
    let scratch = scratch();
    for (name, package_type) in [("Finder.app", Some("FNDR")), ("Untyped.app", None)] {
        let path = scratch.path().join(name);
        write_bundle(&path, package_type);

        let application = inspected(&path);
        assert_eq!(application["bundleId"], BUNDLE_ID, "{name}");
        assert_eq!(application["path"], path.canonicalize().unwrap().to_str().unwrap(), "{name}");
    }
}

#[test]
fn the_app_extension_is_matched_without_regard_to_case() {
    let scratch = scratch();
    let path = scratch.path().join("Shouting.APP");
    write_bundle(&path, Some("APPL"));

    assert_eq!(inspected(&path)["bundleId"], BUNDLE_ID);
}

#[test]
fn a_path_that_is_not_an_application_bundle_is_rejected() {
    let scratch = scratch();
    let plugin = scratch.path().join("Plugin.bundle");
    write_bundle(&plugin, Some("APPL"));
    let file = scratch.path().join("Flat.app");
    fs::write(&file, "").expect("create the file");

    for path in [plugin, file] {
        let diagnostic = rejection(&path);
        assert!(diagnostic.starts_with("recon: not an application: "), "{path:?}: {diagnostic}");
    }
}
