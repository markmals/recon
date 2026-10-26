use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn git(directory: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(directory)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_DATE", "2020-01-02T03:04:05+00:00")
        .env("GIT_COMMITTER_DATE", "2021-03-04T05:06:07+00:00")
        .args(["-c", "user.name=Recon Tests", "-c", "user.email=tests@recon.invalid"])
        .args(arguments)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_directory(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

fn build_and_read_freshness(checkout: &Path, target: &Path) -> bool {
    let output = Command::new(env!("CARGO"))
        .args(["build", "--offline", "-p", "recon", "--manifest-path"])
        .arg(checkout.join("Cargo.toml"))
        .arg("--message-format=json")
        .arg("--target-dir")
        .arg(target)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("RECON_COMMIT", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        .env("RECON_COMMIT_SHORT", "badcafe")
        .env("RECON_COMMIT_DATE", "1900-01-01")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    output
        .stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| line.starts_with(b"{"))
        .map(|line| serde_json::from_slice::<serde_json::Value>(line).unwrap())
        .find(|message| {
            message["reason"] == "compiler-artifact"
                && message["target"]["name"] == "recon"
                && message["executable"].is_string()
        })
        .expect("Recon executable artifact")["fresh"]
        .as_bool()
        .unwrap()
}

fn version(binary: &Path) -> Output {
    let output = Command::new(binary).arg("--version").output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    output
}

fn context_report(binary: &Path) -> serde_json::Value {
    let output = Command::new(binary).arg("context").output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

fn copy_sources(checkout: &Path, package: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"));
    for name in ["Cargo.toml", "Cargo.lock"] {
        fs::copy(source.join("../..").join(name), checkout.join(name)).unwrap();
    }
    for name in ["Cargo.toml", "build.rs"] {
        fs::copy(source.join(name), package.join(name)).unwrap();
    }
    for name in ["src", "build"] {
        copy_directory(&source.join(name), &package.join(name));
    }
}

#[test]
fn proposal0003_committing_overlaid_sources_refreshes_previously_unavailable_metadata() {
    for existing_repository in [false, true] {
        let scratch = tempfile::tempdir().unwrap();
        let checkout = scratch.path().join("checkout");
        let package = checkout.join("crates/recon");
        fs::create_dir_all(&package).unwrap();
        if existing_repository {
            fs::write(package.join("Cargo.toml"), "[package]\nname = \"other\"\n").unwrap();
            git(&checkout, &["init", "-q", "-b", "main"]);
            git(&checkout, &["add", "crates/recon/Cargo.toml"]);
            git(&checkout, &["commit", "-qm", "other package"]);
        }
        copy_sources(&checkout, &package);
        let target = scratch.path().join("target");
        let binary = target.join("debug").join(format!("recon{}", std::env::consts::EXE_SUFFIX));
        build_and_read_freshness(&checkout, &target);
        assert_eq!(
            version(&binary).stdout,
            format!("recon {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
        );
        let context = context_report(&binary);
        assert!(context["recon"]["commit"].is_null(), "{context}");

        if !existing_repository {
            git(&checkout, &["init", "-q", "-b", "main"]);
        }
        git(&checkout, &["add", "--all"]);
        git(&checkout, &["commit", "-qm", "recon sources"]);
        let commit = git(&checkout, &["rev-parse", "--short", "HEAD"]);
        build_and_read_freshness(&checkout, &target);

        assert_eq!(
            version(&binary).stdout,
            format!("recon {} ({commit} 2020-01-02)\n", env!("CARGO_PKG_VERSION")).as_bytes(),
            "existing repository: {existing_repository}"
        );
        assert_eq!(
            context_report(&binary)["recon"]["commit"],
            git(&checkout, &["rev-parse", "HEAD"])
        );
    }
}

#[test]
fn proposal0003_a_changed_git_locator_refreshes_the_baseline_without_source_changes() {
    let scratch = tempfile::tempdir().unwrap();
    let checkout = scratch.path().join("checkout");
    let package = checkout.join("crates/recon");
    fs::create_dir_all(&package).unwrap();
    copy_sources(&checkout, &package);
    let first_git = scratch.path().join("first.git");
    git(
        &checkout,
        &["init", "-q", "-b", "main", "--separate-git-dir", first_git.to_str().unwrap()],
    );
    git(&checkout, &["add", "--all"]);
    git(&checkout, &["commit", "-qm", "first baseline"]);
    git(&checkout, &["pack-refs", "--all", "--no-prune"]);
    let first_commit = git(&checkout, &["rev-parse", "--short", "HEAD"]);
    let target = scratch.path().join("target");
    let binary = target.join("debug").join(format!("recon{}", std::env::consts::EXE_SUFFIX));
    build_and_read_freshness(&checkout, &target);
    assert_eq!(
        version(&binary).stdout,
        format!("recon {} ({first_commit} 2020-01-02)\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );

    let second_git = scratch.path().join("second.git");
    copy_directory(&first_git, &second_git);
    git(
        &checkout,
        &["--git-dir", second_git.to_str().unwrap(), "commit", "--allow-empty", "-qm", "next"],
    );
    fs::write(checkout.join(".git"), format!("gitdir: {}\n", second_git.display())).unwrap();
    let second_commit = git(&checkout, &["rev-parse", "--short", "HEAD"]);
    assert_ne!(second_commit, first_commit);
    build_and_read_freshness(&checkout, &target);
    assert_eq!(
        version(&binary).stdout,
        format!("recon {} ({second_commit} 2020-01-02)\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );
}

fn assert_fresh_rebuild(separate: bool, ref_format: &str) {
    let scratch = tempfile::tempdir().unwrap();
    let checkout = scratch.path().join("checkout");
    let package = checkout.join("crates/recon");
    fs::create_dir_all(&package).unwrap();
    copy_sources(&checkout, &package);
    let metadata = scratch.path().join("metadata.git");
    if separate {
        git(
            &checkout,
            &[
                "init",
                "-q",
                "-b",
                "main",
                "--ref-format",
                ref_format,
                "--separate-git-dir",
                metadata.to_str().unwrap(),
            ],
        );
    } else {
        git(&checkout, &["init", "-q", "-b", "main", "--ref-format", ref_format]);
    }
    git(&checkout, &["add", "--all"]);
    git(&checkout, &["commit", "-qm", "baseline"]);
    if separate {
        git(&checkout, &["pack-refs", "--all"]);
    }
    let target = scratch.path().join("target");
    build_and_read_freshness(&checkout, &target);
    assert!(build_and_read_freshness(&checkout, &target), "unchanged separate={separate}");

    git(&checkout, &["commit", "--allow-empty", "-qm", "next baseline"]);
    let commit = git(&checkout, &["rev-parse", "--short", "HEAD"]);
    build_and_read_freshness(&checkout, &target);
    let binary = target.join("debug").join(format!("recon{}", std::env::consts::EXE_SUFFIX));
    assert_eq!(
        version(&binary).stdout,
        format!("recon {} ({commit} 2020-01-02)\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );
    assert!(build_and_read_freshness(&checkout, &target), "changed separate={separate}");
}

#[test]
fn proposal0003_an_unchanged_ordinary_checkout_stays_fresh() {
    assert_fresh_rebuild(false, "files");
}

#[test]
fn proposal0003_an_unchanged_separate_checkout_stays_fresh() {
    assert_fresh_rebuild(true, "files");
}

#[test]
fn proposal0003_an_unchanged_reftable_checkout_stays_fresh() {
    assert_fresh_rebuild(false, "reftable");
}
