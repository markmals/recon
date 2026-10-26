//! The build script's revision capture, exercised against real Git repositories.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "../build/revision.rs"]
mod revision;

/// Fixed so `RECON_COMMIT_DATE` is predictable; `+00:00` keeps each date on its own calendar day in
/// every timezone. The two differ because the version string reports the author date, so a fixture
/// that committed on the same day could not tell the two apart.
const AUTHORED: &str = "2020-01-02T03:04:05+00:00";
const COMMITTED: &str = "2021-03-04T05:06:07+00:00";

/// Runs Git in a fixture with the ambient user's configuration excluded, so that a global
/// `commit.gpgsign`, `core.hooksPath`, or template directory cannot reach these repositories.
fn git(directory: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(directory)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_DATE", AUTHORED)
        .env("GIT_COMMITTER_DATE", COMMITTED)
        .args(["-c", "user.name=Recon Tests", "-c", "user.email=tests@recon.invalid"])
        .args(arguments)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Unpacks Recon's source layout at `root` and returns the directory the build script runs in.
fn source_archive(root: &Path) -> PathBuf {
    let package = root.join("crates").join("recon");
    fs::create_dir_all(&package).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = [\"crates/recon\"]\n").unwrap();
    fs::write(package.join("Cargo.toml"), "[package]\nname = \"recon\"\n").unwrap();
    package
}

/// A checkout of Recon's source: the archive layout, committed to its own repository.
fn source_checkout(root: &Path) -> PathBuf {
    let package = source_archive(root);
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["add", "--all"]);
    git(root, &["commit", "-qm", "recon source"]);
    package
}

/// An unrelated repository with its own history, standing in for whatever checkout a source archive
/// happens to be unpacked inside.
fn foreign_checkout(root: &Path) {
    fs::create_dir_all(root).unwrap();
    git(root, &["init", "-q", "-b", "main"]);
    fs::write(root.join("README.md"), "not recon\n").unwrap();
    git(root, &["add", "README.md"]);
    git(root, &["commit", "-qm", "unrelated work"]);
}

/// The directives the build script would print for a package directory, one per line.
fn directives(package: &Path) -> Vec<String> {
    let mut sink = Vec::new();
    revision::directives(package, &mut sink).unwrap();
    String::from_utf8(sink).unwrap().lines().map(str::to_owned).collect()
}

#[test]
fn proposal0003_a_source_archive_nested_in_a_foreign_checkout_reports_no_revision() {
    let scratch = tempfile::tempdir().unwrap();
    let foreign = scratch.path().join("workspace");
    foreign_checkout(&foreign);
    let package = source_archive(&foreign.join("vendor").join("recon"));

    let directives = directives(&package);

    assert!(directives.is_empty(), "{directives:?}");
}

#[test]
fn proposal0003_a_source_archive_overlaid_on_a_foreign_root_reports_no_revision() {
    let scratch = tempfile::tempdir().unwrap();
    let foreign = scratch.path().join("workspace");
    foreign_checkout(&foreign);
    let package = source_archive(&foreign);

    let directives = directives(&package);

    assert!(directives.is_empty(), "{directives:?}");
}

#[test]
fn proposal0003_a_staged_source_archive_does_not_inherit_the_foreign_commit() {
    let scratch = tempfile::tempdir().unwrap();
    let foreign = scratch.path().join("workspace");
    foreign_checkout(&foreign);
    let package = source_archive(&foreign);
    git(&foreign, &["add", "--all"]);

    let directives = directives(&package);

    assert!(directives.is_empty(), "{directives:?}");
}

#[test]
fn proposal0003_an_archive_over_an_unrelated_tracked_package_reports_no_revision() {
    for manifest in [
        "[package]\nname = \"other\"\n",
        "[package]\nname = \"unterminated\n",
        "[package]\nname = 17\n",
        "[workspace]\n",
    ] {
        let scratch = tempfile::tempdir().unwrap();
        let foreign = scratch.path().join("workspace");
        foreign_checkout(&foreign);
        let package = foreign.join("crates").join("recon");
        fs::create_dir_all(&package).unwrap();
        fs::write(package.join("Cargo.toml"), manifest).unwrap();
        git(&foreign, &["add", "crates/recon/Cargo.toml"]);
        git(&foreign, &["commit", "-qm", "unrelated package"]);
        source_archive(&foreign);

        let directives = directives(&package);

        assert!(directives.is_empty(), "{manifest:?}: {directives:?}");
    }
}

#[test]
fn proposal0003_a_source_checkout_reports_its_commit_and_watches_its_head() {
    let scratch = tempfile::tempdir().unwrap();
    let checkout = scratch.path().join("recon");
    let package = source_checkout(&checkout);
    fs::write(
        package.join("Cargo.toml"),
        "[package]\nname = \"recon\"\nversion = \"0.2.0\"\n[dependencies]\nserde = \"1\"\n",
    )
    .unwrap();

    let directives = directives(&package);

    let commit = git(&package, &["rev-parse", "HEAD"]);
    let short = git(&package, &["rev-parse", "--short", "HEAD"]);
    for expected in [
        format!("cargo::rustc-env=RECON_COMMIT={commit}"),
        format!("cargo::rustc-env=RECON_COMMIT_SHORT={short}"),
        "cargo::rustc-env=RECON_COMMIT_DATE=2020-01-02".to_owned(),
        format!("cargo::rerun-if-changed={}", git(&package, &["rev-parse", "--git-path", "HEAD"])),
        format!(
            "cargo::rerun-if-changed={}",
            git(&package, &["rev-parse", "--git-path", "refs/heads/main"])
        ),
    ] {
        assert!(directives.contains(&expected), "{expected}: {directives:?}");
    }
}

#[test]
fn proposal0003_a_linked_worktree_reports_the_commit_it_is_checked_out_at() {
    let scratch = tempfile::tempdir().unwrap();
    let checkout = scratch.path().join("recon");
    source_checkout(&checkout);
    let worktree = scratch.path().join("review");
    git(&checkout, &["worktree", "add", "-q", "-b", "review", worktree.to_str().unwrap()]);
    git(&worktree, &["commit", "-q", "--allow-empty", "-m", "review work"]);
    let package = worktree.join("crates").join("recon");

    let directives = directives(&package);

    let commit = git(&package, &["rev-parse", "HEAD"]);
    assert_ne!(commit, git(&checkout, &["rev-parse", "HEAD"]));
    let expected = format!("cargo::rustc-env=RECON_COMMIT={commit}");
    assert!(directives.contains(&expected), "{expected}: {directives:?}");
}

#[test]
fn proposal0003_checkout_path_with_a_trailing_space_keeps_its_revision() {
    let scratch = tempfile::tempdir().unwrap();
    let checkout = scratch.path().join("recon ");
    let package = source_checkout(&checkout);
    let commit = git(&package, &["rev-parse", "HEAD"]);
    assert!(directives(&package).contains(&format!("cargo::rustc-env=RECON_COMMIT={commit}")));
}
