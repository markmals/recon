use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

pub(crate) fn directives(package: &Path, sink: &mut impl Write) -> io::Result<()> {
    if !is_revision_eligible(package) {
        return Ok(());
    }
    for name in ["HEAD", "packed-refs"] {
        if let Some(path) = git(package, &["rev-parse", "--git-path", name]) {
            writeln!(sink, "cargo::rerun-if-changed={path}")?;
        }
    }
    if let Some(reference) = git(package, &["symbolic-ref", "-q", "HEAD"])
        && let Some(path) = git(package, &["rev-parse", "--git-path", &reference])
    {
        writeln!(sink, "cargo::rerun-if-changed={path}")?;
    }
    for (variable, arguments) in [
        ("RECON_COMMIT", ["rev-parse", "HEAD"].as_slice()),
        ("RECON_COMMIT_SHORT", ["rev-parse", "--short", "HEAD"].as_slice()),
        ("RECON_COMMIT_DATE", ["log", "-1", "--format=%as"].as_slice()),
    ] {
        if let Some(value) = git(package, arguments) {
            writeln!(sink, "cargo::rustc-env={variable}={value}")?;
        }
    }
    Ok(())
}

// Root and manifest paths can still match when an archive overlays an unrelated tracked package.
fn is_revision_eligible(package: &Path) -> bool {
    let Some(root) = git(package, &["rev-parse", "--show-toplevel"]) else {
        return false;
    };
    let (Ok(root), Ok(source)) =
        (fs::canonicalize(root), fs::canonicalize(package.join("..").join("..")))
    else {
        return false;
    };
    if root != source {
        return false;
    }
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

fn git(package: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .current_dir(package)
        .env("GIT_NO_LAZY_FETCH", "1")
        .args(arguments)
        .output()
        .ok()?;
    output.status.success().then(|| String::from_utf8(output.stdout).ok())?.map(|mut text| {
        if text.ends_with('\n') {
            text.pop();
        }
        text
    })
}
