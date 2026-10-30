use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) fn directives(package: &Path, sink: &mut impl Write) -> io::Result<()> {
    let eligible = watch_checkout(package, sink)? && head_names_package(package);
    for (variable, arguments) in [
        ("RECON_COMMIT", ["rev-parse", "HEAD"].as_slice()),
        ("RECON_COMMIT_SHORT", ["rev-parse", "--short", "HEAD"].as_slice()),
        ("RECON_COMMIT_DATE", ["log", "-1", "--format=%as"].as_slice()),
    ] {
        let captured = if eligible { git(package, arguments) } else { None };
        // Empty metadata masks ambient RECON_* variables; consumers treat it as unavailable.
        writeln!(sink, "cargo::rustc-env={variable}={}", captured.as_deref().unwrap_or_default())?;
    }
    Ok(())
}

fn watch_checkout(package: &Path, sink: &mut impl Write) -> io::Result<bool> {
    let matching_root = has_matching_workspace_root(package);
    let git_entry = package.join("../../.git");
    // A missing locator detects git init; a locator file detects a worktree being redirected.
    if !git_entry.is_dir() {
        writeln!(sink, "cargo::rerun-if-changed={}", git_entry.display())?;
    }
    if !matching_root {
        return Ok(false);
    }
    // Existing ref stores cover loose, packed, and reftable updates without missing-path rebuilds.
    for name in ["HEAD", "packed-refs", "refs", "reftable"] {
        let Some(path) = git(package, &["rev-parse", "--git-path", name]) else {
            continue;
        };
        let path = PathBuf::from(path);
        let path = if path.is_absolute() { path } else { package.join(path) };
        if path.exists() {
            writeln!(sink, "cargo::rerun-if-changed={}", path.display())?;
        }
    }
    Ok(true)
}

fn has_matching_workspace_root(package: &Path) -> bool {
    let Some(root) = git(package, &["rev-parse", "--show-toplevel"]) else {
        return false;
    };
    let (Ok(root), Ok(source)) =
        (fs::canonicalize(root), fs::canonicalize(package.join("..").join("..")))
    else {
        return false;
    };
    root == source
}

// Root and manifest paths can still match when an archive overlays an unrelated tracked package.
fn head_names_package(package: &Path) -> bool {
    let Some(manifest) = git(package, &["show", "HEAD:./Cargo.toml"]) else {
        return false;
    };
    let Ok(manifest) = toml_edit::Document::parse(manifest) else {
        return false;
    };
    manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml_edit::Item::as_str)
        == Some(env!("CARGO_PKG_NAME"))
}

/// Every inherited `GIT_*` variable is dropped: `GIT_DIR`, `GIT_INDEX_FILE` and `GIT_WORK_TREE`
/// override `current_dir`, and a build started from a Git hook — hk's `pre-push` runs the gate,
/// which builds — would otherwise describe the hook's repository rather than this package's.
fn git(package: &Path, arguments: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command.current_dir(package);
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    let output = command.env("GIT_NO_LAZY_FETCH", "1").args(arguments).output().ok()?;
    output.status.success().then(|| String::from_utf8(output.stdout).ok())?.map(|mut text| {
        if text.ends_with('\n') {
            text.pop();
        }
        text
    })
}
