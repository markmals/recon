//! Compiles this crate's Swift side into a static library and links it into the Rust build.
//!
//! The Swift is Recon's only way to reach the Objective-C frameworks this capability needs, so
//! it is part of the crate rather than a separate artifact: there is no package manager, no
//! network, and nothing extra to install or ship. On a host that is not a Mac the whole step is
//! skipped, which is what keeps the Linux gate building a crate that has Swift in it.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("a manifest directory"));
    let pin = manifest.join("../../swift-toolchain.toml");
    let sources = swift_sources(&manifest.join("swift"));

    println!("cargo:rerun-if-changed={}", pin.display());
    for source in &sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }

    let compiler = Compiler::pinned(&pin);
    let library = compiler
        .build(&sources, &PathBuf::from(env::var_os("OUT_DIR").expect("a build directory")));
    link(&library, &compiler.toolchain);
}

/// The exact compiler every Recon build uses, found and checked before anything is compiled.
struct Compiler {
    /// `.../XcodeDefault.xctoolchain/usr`, which holds the Swift compatibility libraries.
    toolchain: PathBuf,
    sdk: PathBuf,
}

impl Compiler {
    /// # Panics
    ///
    /// When the active compiler is not the pinned one, naming what was found instead. A build
    /// made with another Swift is not the build this pin promises, so it does not happen.
    fn pinned(pin_file: &Path) -> Self {
        let pin = pinned_build(pin_file);
        let version = run("xcrun", &["swiftc".as_ref(), "--version".as_ref()]);
        assert!(
            version.contains(&pin),
            "Recon requires Apple's Swift {pin}, from Xcode 27.1 or the Command Line Tools, as \
             {} pins it. The active compiler is:\n{}",
            pin_file.display(),
            version.trim()
        );
        let swiftc = PathBuf::from(run("xcrun", &["--find".as_ref(), "swiftc".as_ref()]).trim());
        let toolchain = swiftc
            .parent()
            .and_then(Path::parent)
            .expect("the compiler lives in a toolchain")
            .to_path_buf();
        let sdk = PathBuf::from(run("xcrun", &["--show-sdk-path".as_ref()]).trim());
        Self { toolchain, sdk }
    }

    /// The static library, built for exactly the architecture and deployment target Rust links.
    fn build(&self, sources: &[PathBuf], out_dir: &Path) -> PathBuf {
        let library = out_dir.join("librecon_applications_swift.a");
        let mut swiftc = Command::new("xcrun");
        swiftc
            .args(["swiftc", "-emit-library", "-static", "-parse-as-library"])
            .args(["-module-name", "ReconApplications"])
            .args(["-swift-version", "6"])
            .arg(if env::var("OPT_LEVEL").as_deref() == Ok("0") { "-Onone" } else { "-O" })
            .arg("-target")
            .arg(target_triple())
            .arg("-sdk")
            .arg(&self.sdk)
            .arg("-o")
            .arg(&library)
            .args(sources);
        let built = swiftc.status().expect("run the Swift compiler");
        assert!(built.success(), "the Swift compiler refused this crate's sources");
        library
    }
}

/// Where the linker finds the library and the Swift runtime beneath it.
///
/// The Swift object files carry their own linker options, so the runtime libraries themselves
/// need no naming here: macOS supplies them in `/usr/lib/swift`, and the compatibility archives
/// a deployment target may still need come from the toolchain.
fn link(library: &Path, toolchain: &Path) {
    println!(
        "cargo:rustc-link-search=native={}",
        library.parent().expect("a build directory").display()
    );
    println!("cargo:rustc-link-lib=static=recon_applications_swift");
    println!("cargo:rustc-link-search=native=/usr/lib/swift");
    println!("cargo:rustc-link-search=native={}", toolchain.join("lib/swift/macosx").display());
}

/// The `[toolchain] build` of the repository's one Swift pin.
fn pinned_build(pin_file: &Path) -> String {
    let pinned = fs::read_to_string(pin_file)
        .unwrap_or_else(|error| panic!("read {}: {error}", pin_file.display()));
    pinned
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "build")
        .map(|(_, value)| value.trim().trim_matches('"').to_owned())
        .unwrap_or_else(|| panic!("{} declares no build = \"…\" compiler", pin_file.display()))
}

/// The Swift target Rust is linking for: its architecture, and the oldest macOS it runs on.
fn target_triple() -> String {
    let architecture = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok(other) => return format!("{other}-apple-macos{}", deployment_target("10.12")),
        Err(error) => panic!("read the target architecture: {error}"),
    };
    format!("{architecture}-apple-macos{}", deployment_target("11.0"))
}

/// Rust's own deployment target for this build, which the Swift must match exactly or the
/// linker warns that the two halves of one binary target different systems.
fn deployment_target(default: &str) -> String {
    env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| default.to_owned())
}

fn swift_sources(directory: &Path) -> Vec<PathBuf> {
    let mut sources: Vec<PathBuf> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()))
        .map(|entry| entry.expect("a readable directory entry").path())
        .filter(|path| path.extension() == Some(OsStr::new("swift")))
        .collect();
    assert!(!sources.is_empty(), "{} holds no Swift sources", directory.display());
    sources.sort();
    sources
}

fn run(program: &str, arguments: &[&OsStr]) -> String {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .unwrap_or_else(|error| panic!("run {program}: {error}"));
    assert!(
        output.status.success(),
        "{program} {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("UTF-8 output")
}
