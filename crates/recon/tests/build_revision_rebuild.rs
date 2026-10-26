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

fn build(checkout: &Path, target: &Path) {
    let output = Command::new(env!("CARGO"))
        .args(["build", "--offline", "-p", "recon", "--manifest-path"])
        .arg(checkout.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(target)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("RECON_COMMIT")
        .env_remove("RECON_COMMIT_SHORT")
        .env_remove("RECON_COMMIT_DATE")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
}

fn version(binary: &Path) -> Output {
    let output = Command::new(binary).arg("--version").output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    output
}

#[test]
fn proposal0003_committing_overlaid_sources_refreshes_previously_unavailable_metadata() {
    let scratch = tempfile::tempdir().unwrap();
    let checkout = scratch.path().join("checkout");
    let package = checkout.join("crates/recon");
    fs::create_dir_all(&package).unwrap();
    fs::write(package.join("Cargo.toml"), "[package]\nname = \"other\"\n").unwrap();
    git(&checkout, &["init", "-q", "-b", "main"]);
    git(&checkout, &["add", "crates/recon/Cargo.toml"]);
    git(&checkout, &["commit", "-qm", "other package"]);

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
    let target = scratch.path().join("target");
    let binary = target.join("debug").join(format!("recon{}", std::env::consts::EXE_SUFFIX));
    build(&checkout, &target);
    assert_eq!(
        version(&binary).stdout,
        format!("recon {}\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );

    git(&checkout, &["add", "--all"]);
    git(&checkout, &["commit", "-qm", "recon sources"]);
    let commit = git(&checkout, &["rev-parse", "--short", "HEAD"]);
    build(&checkout, &target);

    assert_eq!(
        version(&binary).stdout,
        format!("recon {} ({commit} 2020-01-02)\n", env!("CARGO_PKG_VERSION")).as_bytes()
    );
}
