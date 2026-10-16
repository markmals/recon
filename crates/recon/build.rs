//! Embeds an Info.plist in the `recon` executable on macOS. macOS records a privacy grant against
//! the executable's code-signing identity, and System Settings names the client after this plist,
//! so `recon` needs one even though it is a bare executable rather than an application bundle.

use std::env;
use std::fs;
use std::path::PathBuf;

const BUNDLE_IDENTIFIER: &str = "me.malstrom.recon";
const BUNDLE_NAME: &str = "recon";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let version = env::var("CARGO_PKG_VERSION").expect("cargo sets CARGO_PKG_VERSION");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let plist = out_dir.join("Info.plist");
    fs::write(&plist, info_plist(&version)).expect("write the embedded Info.plist");

    let plist = plist.to_str().expect("a UTF-8 OUT_DIR");
    println!("cargo::rustc-link-arg-bin=recon=-Wl,-sectcreate,__TEXT,__info_plist,{plist}");
}

fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>{BUNDLE_IDENTIFIER}</string>
    <key>CFBundleName</key>
    <string>{BUNDLE_NAME}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
</dict>
</plist>
"#
    )
}
