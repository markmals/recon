#![cfg(target_os = "macos")]

#[path = "../src/macos/inspection_access.rs"]
mod inspection_access;

use std::path::PathBuf;

use inspection_access::{
    AccessMode, AccessProfile, CommandResult, CommandRunner, Eligibility, Fact, HelperFacts,
    HostFacts, TargetFacts, TargetSelection, add_research_boot_args, collect_preflight, evaluate,
    prepare_host, remove_research_boot_args, revert_host,
};
use proptest::prelude::*;

fn known<T>(value: T) -> Fact<T> {
    Fact::known(value, "test fixture")
}

fn target() -> TargetFacts {
    TargetFacts {
        bundle_path: PathBuf::from("/Applications/Fixture.app"),
        bundle_id: Some("org.example.fixture".into()),
        executable_path: known("/Applications/Fixture.app/Contents/MacOS/Fixture".into()),
        pid: Some(42),
        running_pids: vec![42],
        process_executable: known("/Applications/Fixture.app/Contents/MacOS/Fixture".into()),
        architecture: known("arm64".into()),
        executable_architectures: known(vec!["arm64".into()]),
        signed: known(true),
        apple_signed: known(false),
        get_task_allow: known(true),
        allow_dyld_environment_variables: known(false),
        hardened_runtime: known(false),
        library_validation: known(false),
        platform_binary: known(false),
    }
}

fn host() -> HostFacts {
    HostFacts {
        os_version: known("27.0.1".into()),
        architecture: known("arm64".into()),
        sip_disabled: known(false),
        amfi_disabled: known(false),
        active_boot_args: known(vec![]),
        configured_boot_args: known(vec![]),
    }
}

fn helper() -> HelperFacts {
    HelperFacts {
        local_available: known(true),
        local_compatible: known(true),
        resident: known(true),
        resident_compatible: known(true),
        attach_without_dialog: known(true),
        inspector_thread_set_state: known(true),
    }
}

fn state(
    t: &TargetFacts,
    h: &HostFacts,
    x: &HelperFacts,
    mode: AccessMode,
    profile: AccessProfile,
) -> Eligibility {
    evaluate(t, h, x)
        .into_iter()
        .find(|decision| decision.mode == mode && decision.profile == profile)
        .unwrap()
        .status
}

#[test]
fn proposal0005_standard_cooperative_attach_is_eligible_without_research_preparation() {
    assert_eq!(
        state(&target(), &host(), &helper(), AccessMode::Attach, AccessProfile::Standard),
        Eligibility::Eligible
    );
}

#[test]
fn proposal0005_noncooperative_attach_requires_explicit_prepared_research_host() {
    let mut t = target();
    t.get_task_allow = known(false);
    assert_eq!(
        state(&t, &host(), &helper(), AccessMode::Attach, AccessProfile::Standard),
        Eligibility::Blocked
    );
    assert_eq!(
        state(&t, &host(), &helper(), AccessMode::Attach, AccessProfile::Research),
        Eligibility::Blocked
    );
    let mut h = host();
    h.sip_disabled = known(true);
    h.amfi_disabled = known(true);
    h.active_boot_args = known(vec![
        "amfi_get_out_of_my_way=0x1".into(),
        "-arm64e_preview_abi".into(),
        "tss_should_crash=0".into(),
    ]);
    assert_eq!(
        state(&t, &h, &helper(), AccessMode::Attach, AccessProfile::Research),
        Eligibility::Eligible
    );
    let reasons = evaluate(&t, &host(), &helper());
    assert!(
        reasons
            .iter()
            .find(|d| d.mode == AccessMode::Attach && d.profile == AccessProfile::Research)
            .unwrap()
            .reasons
            .iter()
            .any(|r| r.contains("prepare-host"))
    );
}

#[test]
fn proposal0005_first_party_requires_thread_state_entitlement_and_prepared_host() {
    let mut t = target();
    t.apple_signed = known(true);
    t.platform_binary = known(true);
    let mut h = host();
    h.sip_disabled = known(true);
    h.amfi_disabled = known(true);
    h.active_boot_args = known(vec![
        "amfi_get_out_of_my_way=0x1".into(),
        "-arm64e_preview_abi".into(),
        "tss_should_crash=0".into(),
    ]);
    let mut x = helper();
    assert_eq!(
        state(&t, &h, &x, AccessMode::Attach, AccessProfile::Standard),
        Eligibility::Blocked
    );
    x.inspector_thread_set_state = known(false);
    assert_eq!(
        state(&t, &h, &x, AccessMode::Attach, AccessProfile::Research),
        Eligibility::Blocked
    );
    x.inspector_thread_set_state = known(true);
    assert_eq!(
        state(&t, &h, &x, AccessMode::Attach, AccessProfile::Research),
        Eligibility::Eligible
    );
}

#[test]
fn proposal0005_inaccessible_facts_are_unknown_not_negative_and_known_denials_dominate() {
    let mut t = target();
    t.get_task_allow = Fact::unknown("codesign", "access denied");
    assert_eq!(
        state(&t, &host(), &helper(), AccessMode::Attach, AccessProfile::Standard),
        Eligibility::Unknown
    );
    t.architecture = Fact::unknown("ps arch", "access denied");
    assert_eq!(
        state(&t, &host(), &helper(), AccessMode::Attach, AccessProfile::Standard),
        Eligibility::Unknown
    );
    let mut h = host();
    h.sip_disabled = Fact::unknown("csrutil", "access denied");
    t.get_task_allow = known(false);
    assert_eq!(
        state(&t, &h, &helper(), AccessMode::Attach, AccessProfile::Research),
        Eligibility::Blocked
    );
}

#[test]
fn proposal0005_modes_never_fallback_or_launch_a_running_process() {
    let mut x = helper();
    x.resident = known(false);
    assert_eq!(
        state(&target(), &host(), &x, AccessMode::Existing, AccessProfile::Standard),
        Eligibility::Blocked
    );
    assert_eq!(
        state(&target(), &host(), &x, AccessMode::Attach, AccessProfile::Standard),
        Eligibility::Eligible
    );
    assert_eq!(
        state(&target(), &host(), &helper(), AccessMode::Launch, AccessProfile::Standard),
        Eligibility::Blocked
    );
    let mut t = target();
    t.pid = None;
    t.running_pids.clear();
    assert_eq!(
        state(&t, &host(), &helper(), AccessMode::Attach, AccessProfile::Standard),
        Eligibility::Blocked
    );
    assert_eq!(
        state(&t, &host(), &helper(), AccessMode::Launch, AccessProfile::Standard),
        Eligibility::Eligible
    );
}

#[test]
fn proposal0005_unsupported_architecture_and_version_are_blocked() {
    let mut t = target();
    t.architecture = known("x86_64".into());
    assert_eq!(
        state(&t, &host(), &helper(), AccessMode::Attach, AccessProfile::Research),
        Eligibility::Blocked
    );
    let mut h = host();
    h.os_version = known("26.1".into());
    assert_eq!(
        state(&target(), &h, &helper(), AccessMode::Existing, AccessProfile::Standard),
        Eligibility::Blocked
    );
}

#[test]
fn proposal0005_boot_args_add_remove_preserves_unrelated_arguments() {
    let before = "keepsyms=1 debug=0x14";
    let applied = add_research_boot_args(before);
    assert_eq!(
        applied,
        "keepsyms=1 debug=0x14 amfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0"
    );
    assert_eq!(add_research_boot_args(&applied), applied);
    assert_eq!(remove_research_boot_args(&applied), before);
    assert_eq!(
        remove_research_boot_args(
            "amfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0"
        ),
        ""
    );
}

proptest! {
    #[test]
    fn proposal0005_boot_arg_revert_preserves_every_unrelated_token(tokens in proptest::collection::vec("[a-z][a-z0-9_]{0,12}=[a-z0-9]{1,8}", 0..24)) {
        let original = tokens.join(" ");
        prop_assert_eq!(remove_research_boot_args(&add_research_boot_args(&original)), original);
    }
}

#[derive(Default)]
struct FakeRunner {
    invocations: Vec<(String, Vec<String>)>,
    responses: Vec<Result<CommandResult, String>>,
    kernel_status: Option<Result<u32, String>>,
    kernel_hash: Option<Result<[u8; 20], String>>,
    process_path: Option<Result<String, String>>,
    process_arch: Option<Result<String, String>>,
}

impl CommandRunner for FakeRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandResult, String> {
        self.invocations.push((program.to_owned(), args.iter().map(|s| (*s).to_owned()).collect()));
        if self.responses.is_empty() {
            return Err("unavailable".into());
        }
        self.responses.remove(0)
    }

    fn process_signing_status(&mut self, _pid: u32) -> Result<u32, String> {
        self.kernel_status.take().unwrap_or_else(|| Err("kernel status unavailable".into()))
    }

    fn process_cdhash(&mut self, _pid: u32) -> Result<[u8; 20], String> {
        self.kernel_hash.take().unwrap_or_else(|| Err("kernel hash unavailable".into()))
    }

    fn process_executable(&mut self, _pid: u32) -> Result<String, String> {
        self.process_path.take().unwrap_or_else(|| Err("process path unavailable".into()))
    }

    fn process_architecture(&mut self, _pid: u32) -> Result<String, String> {
        self.process_arch.take().unwrap_or_else(|| Err("process architecture unavailable".into()))
    }
}

fn ok(stdout: &str) -> Result<CommandResult, String> {
    Ok(CommandResult { success: true, stdout: stdout.into(), stderr: String::new() })
}

#[test]
fn proposal0005_preflight_collects_unknowns_without_sudo_or_host_effects() {
    let mut runner = FakeRunner::default();
    let report = collect_preflight(
        &mut runner,
        TargetSelection {
            bundle_path: "/Applications/Fixture.app".into(),
            bundle_id: Some("org.example.fixture".into()),
            executable_path: Some("/Applications/Fixture.app/Contents/MacOS/Fixture".into()),
            pid: Some(42),
            running_pids: vec![42],
        },
        helper(),
    );
    assert_eq!(report.target.get_task_allow.value, None);
    assert_eq!(report.host.sip_disabled.value, None);
    assert!(runner.invocations.iter().all(|(program, args)| !program.contains("sudo")
        && !args.iter().any(|arg| arg.contains("sudo"))
        && !(program.contains("nvram") && args.iter().any(|arg| arg.contains('=')))));
    assert_eq!(report.decisions.len(), 6);
}

#[test]
fn proposal0005_prepare_host_is_the_only_command_that_can_invoke_sudo_nvram() {
    let mut runner =
        FakeRunner { responses: vec![ok("boot-args\tkeepsyms=1\n"), ok("")], ..Default::default() };
    let plan = prepare_host(&mut runner).unwrap();
    assert_eq!(
        plan.boot_args,
        "keepsyms=1 amfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0"
    );
    assert_eq!(runner.invocations[0].0, "/usr/sbin/nvram");
    assert_eq!(runner.invocations[1], ("/usr/bin/sudo".into(), vec!["/usr/sbin/nvram".into(), "boot-args=keepsyms=1 amfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0".into()]));
    assert!(plan.operator_steps.contains("csrutil disable"));
    assert!(plan.operator_steps.contains("Recovery"));
    assert!(plan.operator_steps.contains("reboot"));
    let mut runner = FakeRunner {
        responses: vec![
            ok(
                "boot-args\tkeepsyms=1 amfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0\n",
            ),
            ok(""),
        ],
        ..Default::default()
    };
    let reverted = revert_host(&mut runner).unwrap();
    assert_eq!(reverted.boot_args, "keepsyms=1");
    assert_eq!(runner.invocations[1].1, vec!["/usr/sbin/nvram", "boot-args=keepsyms=1"]);
}

#[test]
fn proposal0005_unexpected_nvram_output_cannot_authorize_boot_arg_write() {
    let mut runner =
        FakeRunner { responses: vec![ok("boot-args-malformed"), ok("")], ..Default::default() };
    assert!(prepare_host(&mut runner).is_err());
    assert_eq!(runner.invocations.len(), 1);
}

#[test]
fn proposal0005_revert_with_no_recon_boot_args_does_not_request_authorization() {
    let mut runner = FakeRunner {
        responses: vec![Ok(CommandResult {
            success: false,
            stdout: String::new(),
            stderr: "nvram: data was not found".into(),
        })],
        ..Default::default()
    };
    assert_eq!(revert_host(&mut runner).unwrap().boot_args, "");
    assert_eq!(runner.invocations.len(), 1);
}
#[test]
fn proposal0005_revert_deletes_boot_args_variable_only_when_no_unrelated_arg_remains() {
    let mut runner = FakeRunner {
        responses: vec![
            ok("boot-args\tamfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0"),
            ok(""),
        ],
        ..Default::default()
    };
    assert_eq!(revert_host(&mut runner).unwrap().boot_args, "");
    assert_eq!(runner.invocations[1].1, vec!["/usr/sbin/nvram", "-d", "boot-args"]);
}

#[test]
fn proposal0005_preflight_reads_actual_process_and_signing_facts_with_sources() {
    let mut runner = FakeRunner {
        process_path: Some(Ok("/Applications/Fixture.app/Contents/MacOS/Fixture".into())),
        process_arch: Some(Ok("arm64e".into())),
        kernel_status: Some(Ok(0x2000_0001 | 0x0001_0000 | 0x0000_2000 | 0x0400_0000)),
        kernel_hash: Some(Ok([0xaa; 20])),
        responses: vec![
        ok("27.0.1\n"),
        ok("arm64\n"),
        ok("System Integrity Protection status: disabled.\n"),
        ok("amfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0\n"),
        ok("boot-args\tamfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0\n"),
        ok("arm64 arm64e\n"),
        Ok(CommandResult { success: true, stdout: String::new(), stderr: "Executable=/Applications/Fixture.app/Contents/MacOS/Fixture\nIdentifier=com.apple.Fixture\nAuthority=Apple Mac OS Application Signing\nCodeDirectory v=20500 size=123 flags=0x12000(runtime,library-validation)\nCDHash=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n".into() }),
        ok("<?xml version=\"1.0\" encoding=\"UTF-8\"?><!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\"><plist version=\"1.0\"><dict><key>com.apple.security.get-task-allow</key><false/></dict></plist>"),
    ], ..Default::default() };
    let report = collect_preflight(
        &mut runner,
        TargetSelection {
            bundle_path: "/Applications/Fixture.app".into(),
            bundle_id: Some("com.apple.Fixture".into()),
            executable_path: Some("/Applications/Fixture.app/Contents/MacOS/Fixture".into()),
            pid: Some(42),
            running_pids: vec![42],
        },
        helper(),
    );
    assert_eq!(report.target.architecture.value.as_deref(), Some("arm64e"));
    assert_eq!(report.target.apple_signed.value, Some(true));
    assert_eq!(report.target.get_task_allow.value, Some(false));
    assert_eq!(report.target.hardened_runtime.value, Some(true));
    assert_eq!(report.target.library_validation.value, Some(true));
    assert_eq!(report.target.platform_binary.value, Some(true));
    assert!(report.target.platform_binary.source.contains("csops"));
    assert_eq!(report.host.sip_disabled.value, Some(true));
    assert_eq!(report.host.amfi_disabled.value, Some(true));
    assert!(report.target.get_task_allow.source.contains("csops"));
    assert_eq!(&runner.invocations[6].1[..3], ["--display", "--architecture", "arm64e"]);
    assert!(runner.invocations.last().unwrap().1.contains(&"-".into()));
    assert!(runner.invocations.iter().all(|(program, _)| program != "/usr/bin/sudo"));
}

#[test]
fn proposal0005_process_image_mismatch_blocks_all_modes_without_trusting_disk_signature() {
    let mut t = target();
    t.process_executable = known("/Applications/Other.app/Contents/MacOS/Other".into());
    assert!(evaluate(&t, &host(), &helper()).iter().all(|d| d.status == Eligibility::Blocked));
}

#[test]
fn proposal0005_prepare_host_status_is_read_only_and_reports_each_prerequisite() {
    let mut runner = FakeRunner {
        responses: vec![
            ok("27.0.1\n"),
            ok("arm64\n"),
            ok("System Integrity Protection status: enabled.\n"),
            ok("\n"),
            ok("boot-args\tkeepsyms=1\n"),
        ],
        ..Default::default()
    };
    let status = inspection_access::prepare_host_status(&mut runner);
    assert_eq!(status.host.sip_disabled.value, Some(false));
    assert_eq!(status.host.amfi_disabled.value, Some(false));
    assert_eq!(status.host.configured_boot_args.value, Some(vec!["keepsyms=1".into()]));
    assert_eq!(
        status.required_boot_args,
        "keepsyms=1 amfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0"
    );
    assert_eq!(status.readiness, Eligibility::Blocked);
    assert!(runner.invocations.iter().all(|(program, _)| program != "/usr/bin/sudo"));
}

#[test]
fn proposal0005_hardened_launch_requires_dyld_entitlement_not_absent_hardening() {
    let mut t = target();
    t.pid = None;
    t.running_pids.clear();
    t.hardened_runtime = known(true);
    t.allow_dyld_environment_variables = known(true);
    assert_eq!(
        state(&t, &host(), &helper(), AccessMode::Launch, AccessProfile::Standard),
        Eligibility::Eligible
    );
    t.allow_dyld_environment_variables = known(false);
    assert_eq!(
        state(&t, &host(), &helper(), AccessMode::Launch, AccessProfile::Standard),
        Eligibility::Blocked
    );
}

#[test]
fn proposal0005_preflight_report_uses_cli_camel_case_fields() {
    let report = inspection_access::PreflightReport {
        target: target(),
        host: host(),
        helper: helper(),
        decisions: evaluate(&target(), &host(), &helper()),
    };
    let json = serde_json::to_value(&report).unwrap();
    assert!(json["target"]["bundlePath"].is_string());
    assert_eq!(json["target"]["getTaskAllow"]["value"], true);
    assert_eq!(json["host"]["sipDisabled"]["value"], false);
    assert!(json["helper"]["inspectorThreadSetState"].is_object());
    assert!(json["decisions"][0]["evidenceSources"].is_array());
}

#[test]
fn proposal0005_unknown_process_protections_do_not_fall_back_to_disk_flags() {
    let mut runner = FakeRunner {
        process_path: Some(Ok("/Applications/Fixture.app/Contents/MacOS/Fixture".into())),
        process_arch: Some(Ok("arm64".into())),
        responses: vec![
        ok("27.0.1\n"), ok("arm64\n"),
        ok("System Integrity Protection status: disabled.\n"),
        ok("amfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0\n"),
        ok("boot-args\tamfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0\n"),
        ok("arm64\n"),
        Ok(CommandResult { success: true, stdout: String::new(), stderr: "Authority=Developer ID Application: Example\nCodeDirectory v=20500 flags=0x12000(runtime,library-validation)\n".into() }),
        ok("<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict><key>com.apple.security.get-task-allow</key><false/></dict></plist>"),
    ], ..Default::default() };
    let report = collect_preflight(
        &mut runner,
        TargetSelection {
            bundle_path: "/Applications/Fixture.app".into(),
            bundle_id: Some("org.example.fixture".into()),
            executable_path: Some("/Applications/Fixture.app/Contents/MacOS/Fixture".into()),
            pid: Some(42),
            running_pids: vec![42],
        },
        helper(),
    );
    assert_eq!(report.target.get_task_allow.value, None);
    assert_eq!(report.target.hardened_runtime.value, None);
    assert_eq!(report.target.platform_binary.value, None);
}

#[test]
fn proposal0005_running_architecture_comes_from_kernel_process_not_unavailable_ps_format() {
    assert_eq!(
        inspection_access::decode_process_architecture(0x0100_000c, 2).as_deref(),
        Some("arm64e")
    );
    assert_eq!(
        inspection_access::decode_process_architecture(0x0100_000c, 12).as_deref(),
        Some("arm64e.x1")
    );
    assert_eq!(
        inspection_access::decode_process_architecture(0x0100_000c, 0).as_deref(),
        Some("arm64")
    );
    assert_eq!(
        inspection_access::decode_process_architecture(0x0100_0007, 3).as_deref(),
        Some("x86_64")
    );
    assert_eq!(inspection_access::decode_process_architecture(1234, 0), None);
}

#[test]
fn proposal0005_arm64e_x1_platform_target_remains_research_eligible() {
    let mut t = target();
    t.architecture = known("arm64e.x1".into());
    t.apple_signed = known(true);
    t.platform_binary = known(true);
    let mut h = host();
    h.sip_disabled = known(true);
    h.amfi_disabled = known(true);
    h.active_boot_args = known(vec![
        "amfi_get_out_of_my_way=0x1".into(),
        "-arm64e_preview_abi".into(),
        "tss_should_crash=0".into(),
    ]);
    assert_eq!(
        state(&t, &h, &helper(), AccessMode::Attach, AccessProfile::Research),
        Eligibility::Eligible
    );
}

#[test]
fn proposal0005_preflight_chooses_only_unambiguous_running_process() {
    let mut runner = FakeRunner::default();
    let selection = TargetSelection {
        bundle_path: "/Applications/Fixture.app".into(),
        bundle_id: Some("org.example.fixture".into()),
        executable_path: None,
        pid: None,
        running_pids: vec![42],
    };
    let report = collect_preflight(&mut runner, selection.clone(), helper());
    assert_eq!(report.target.pid, Some(42));
    let mut runner = FakeRunner::default();
    let report = collect_preflight(
        &mut runner,
        TargetSelection { running_pids: vec![42, 43], ..selection },
        helper(),
    );
    assert_eq!(report.target.pid, None);
}

#[test]
fn proposal0005_system_runner_reads_own_live_process_architecture_and_executable() {
    let pid = std::process::id();
    let mut runner = inspection_access::SystemRunner;
    let actual = runner.process_executable(pid).unwrap();
    assert_eq!(
        std::fs::canonicalize(actual).unwrap(),
        std::env::current_exe().unwrap().canonicalize().unwrap()
    );
    assert!(matches!(runner.process_architecture(pid).as_deref(), Ok("arm64" | "arm64e")));
}

#[test]
fn proposal0005_system_runner_reads_own_live_kernel_signing_status() {
    let mut runner = inspection_access::SystemRunner;
    let flags = runner.process_signing_status(std::process::id()).unwrap();
    assert_ne!(flags, 0);
}

#[test]
fn proposal0005_kernel_signing_rejects_out_of_range_pid_without_querying_another_process() {
    let mut runner = inspection_access::SystemRunner;
    assert!(runner.process_signing_status(u32::MAX).unwrap_err().contains("outside pid_t range"));
}

#[test]
fn proposal0005_host_preparation_status_never_turns_unreadable_security_facts_into_false() {
    let mut runner = FakeRunner::default();
    let status = inspection_access::prepare_host_status(&mut runner);
    assert_eq!(status.readiness, Eligibility::Unknown);
    assert_eq!(status.host.sip_disabled.value, None);
    assert_eq!(status.host.amfi_disabled.value, None);
    assert!(status.required_boot_args.contains("unavailable"));
}

#[test]
fn proposal0005_disk_signature_is_unknown_when_it_does_not_match_live_process() {
    let mut runner = FakeRunner {
        process_path: Some(Ok("/Applications/Fixture.app/Contents/MacOS/Fixture".into())),
        process_arch: Some(Ok("arm64e".into())),
        kernel_status: Some(Ok(0x2000_0001)),
        kernel_hash: Some(Ok([0xbb; 20])),
        responses: vec![
            ok("27.0.1"), ok("arm64"), ok("System Integrity Protection status: disabled."),
            ok("amfi_get_out_of_my_way=0x1"), ok("boot-args\tamfi_get_out_of_my_way=0x1"),
            ok("arm64e"),
            Ok(CommandResult { success: true, stdout: String::new(), stderr: "Authority=Apple Code Signing Certification Authority\nCodeDirectory v=20500 flags=0x0(none)\nCDHash=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n".into() }),
            ok("<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict><key>com.apple.security.cs.allow-dyld-environment-variables</key><true/></dict></plist>"),
        ], ..Default::default()
    };
    let report = collect_preflight(
        &mut runner,
        TargetSelection {
            bundle_path: "/Applications/Fixture.app".into(),
            bundle_id: Some("com.apple.fixture".into()),
            executable_path: Some("/Applications/Fixture.app/Contents/MacOS/Fixture".into()),
            pid: Some(42),
            running_pids: vec![42],
        },
        helper(),
    );
    assert_eq!(report.target.apple_signed.value, None);
    assert_eq!(report.target.allow_dyld_environment_variables.value, None);
    assert_eq!(report.target.platform_binary.value, Some(false));
}

#[test]
fn proposal0005_signed_executable_with_no_entitlements_has_known_absence() {
    let mut runner = FakeRunner { responses: vec![
        ok("27.0.1"), ok("arm64"),
        ok("System Integrity Protection status: enabled."), ok(""), ok("boot-args\tkeepsyms=1"),
        ok("arm64"),
        Ok(CommandResult { success: true, stdout: String::new(), stderr: "Authority=Developer ID Application: Example\nCodeDirectory v=20500 flags=0x10000(runtime)\n".into() }),
        ok(""),
    ], ..Default::default() };
    let report = collect_preflight(
        &mut runner,
        TargetSelection {
            bundle_path: "/Applications/Fixture.app".into(),
            bundle_id: Some("org.example.fixture".into()),
            executable_path: Some("/Applications/Fixture.app/Contents/MacOS/Fixture".into()),
            pid: None,
            running_pids: vec![],
        },
        helper(),
    );
    assert_eq!(report.target.allow_dyld_environment_variables.value, Some(false));
    assert_eq!(report.target.get_task_allow.value, Some(false));
    assert_eq!(
        state(
            &report.target,
            &report.host,
            &report.helper,
            AccessMode::Launch,
            AccessProfile::Standard
        ),
        Eligibility::Blocked
    );
}
