use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;

pub(crate) fn directives(package: &Path, sink: &mut impl Write) -> io::Result<()> {
    if !is_source_checkout(package) {
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

/// Whether the repository Git finds from `package` is Recon's own checkout rather than one the
/// source was unpacked inside. Recon's source sits at `crates/recon` under the workspace root, so
/// a Git root anywhere else merely encloses it, and a root whose `HEAD` lacks this manifest has
/// had it overlaid; both would otherwise report a stranger's commit as this build's revision.
fn is_source_checkout(package: &Path) -> bool {
    let Some(root) = git(package, &["rev-parse", "--show-toplevel"]) else {
        return false;
    };
    let (Ok(root), Ok(source)) =
        (fs::canonicalize(root), fs::canonicalize(package.join("..").join("..")))
    else {
        return false;
    };
    root == source && git(package, &["cat-file", "-e", "HEAD:./Cargo.toml"]).is_some()
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
