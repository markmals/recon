#[cfg(unix)]
#[test]
fn malformed_target_argument_reports_a_cli_error_instead_of_panicking() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use std::process::Command;

    let output = Command::new(env!("CARGO_BIN_EXE_recon"))
        .args([OsString::from("app"), OsString::from("inspect"), OsString::from_vec(vec![0xff])])
        .output()
        .expect("start recon");

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 diagnostic");
    assert!(stderr.contains("recon: invalid UTF-8 argument"), "{stderr}");
    assert!(!stderr.contains("panicked at"), "{stderr}");
}
