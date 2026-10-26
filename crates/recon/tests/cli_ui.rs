#![cfg(target_os = "macos")]

use std::process::Command;

#[cfg(target_os = "macos")]
#[test]
fn the_executable_identifies_itself_to_macos_privacy_settings_as_recon() {
    let output = Command::new("/bin/launchctl")
        .args(["plist", "__TEXT,__info_plist", env!("CARGO_BIN_EXE_recon")])
        .output()
        .expect("start launchctl");

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let embedded = String::from_utf8(output.stdout).expect("UTF-8 property list");
    assert!(embedded.contains(r#""CFBundleIdentifier" = "me.malstrom.recon";"#), "{embedded}");
    assert!(embedded.contains(r#""CFBundleName" = "recon";"#), "{embedded}");
}
