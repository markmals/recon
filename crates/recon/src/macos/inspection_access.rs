use std::path::{Path, PathBuf};
use std::process::Command;

use schemars::JsonSchema;
use serde::Serialize;
use std::ffi::CStr;

#[derive(Clone, Debug, Serialize, JsonSchema)]
pub(crate) struct Fact<T> {
    pub(crate) value: Option<T>,
    pub(crate) source: String,
    pub(crate) detail: Option<String>,
}

impl<T> Fact<T> {
    pub(crate) fn known(value: T, source: &str) -> Self {
        Self { value: Some(value), source: source.into(), detail: None }
    }
    pub(crate) fn unknown(source: &str, detail: &str) -> Self {
        Self { value: None, source: source.into(), detail: Some(detail.into()) }
    }
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TargetSelection {
    pub(crate) bundle_path: PathBuf,
    pub(crate) bundle_id: Option<String>,
    pub(crate) executable_path: Option<PathBuf>,
    pub(crate) pid: Option<u32>,
    pub(crate) running_pids: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TargetFacts {
    pub(crate) bundle_path: PathBuf,
    pub(crate) bundle_id: Option<String>,
    pub(crate) executable_path: Fact<String>,
    pub(crate) pid: Option<u32>,
    pub(crate) running_pids: Vec<u32>,
    pub(crate) process_executable: Fact<String>,
    pub(crate) architecture: Fact<String>,
    pub(crate) executable_architectures: Fact<Vec<String>>,
    pub(crate) signed: Fact<bool>,
    pub(crate) apple_signed: Fact<bool>,
    pub(crate) get_task_allow: Fact<bool>,
    pub(crate) allow_dyld_environment_variables: Fact<bool>,
    pub(crate) hardened_runtime: Fact<bool>,
    pub(crate) library_validation: Fact<bool>,
    pub(crate) platform_binary: Fact<bool>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostFacts {
    pub(crate) os_version: Fact<String>,
    pub(crate) architecture: Fact<String>,
    pub(crate) sip_disabled: Fact<bool>,
    pub(crate) amfi_disabled: Fact<bool>,
    pub(crate) active_boot_args: Fact<Vec<String>>,
    pub(crate) configured_boot_args: Fact<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HelperFacts {
    pub(crate) local_available: Fact<bool>,
    pub(crate) local_compatible: Fact<bool>,
    pub(crate) resident: Fact<bool>,
    pub(crate) resident_compatible: Fact<bool>,
    pub(crate) attach_without_dialog: Fact<bool>,
    pub(crate) inspector_thread_set_state: Fact<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AccessMode {
    Existing,
    Launch,
    Attach,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AccessProfile {
    Standard,
    Research,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Eligibility {
    Eligible,
    Blocked,
    Unknown,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModeDecision {
    pub(crate) mode: AccessMode,
    pub(crate) profile: AccessProfile,
    pub(crate) status: Eligibility,
    pub(crate) reasons: Vec<String>,
    pub(crate) evidence_sources: Vec<String>,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreflightReport {
    pub(crate) target: TargetFacts,
    pub(crate) host: HostFacts,
    pub(crate) helper: HelperFacts,
    pub(crate) decisions: Vec<ModeDecision>,
}

#[derive(Clone, Debug)]
pub(crate) struct CommandResult {
    pub(crate) success: bool,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}
pub(crate) trait CommandRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandResult, String>;
    fn process_signing_status(&mut self, _pid: u32) -> Result<u32, String> {
        Err("kernel signing query unavailable".into())
    }
    fn process_cdhash(&mut self, _pid: u32) -> Result<[u8; 20], String> {
        Err("kernel CodeDirectory hash unavailable".into())
    }
    fn process_executable(&mut self, _pid: u32) -> Result<String, String> {
        Err("process path unavailable".into())
    }
    fn process_architecture(&mut self, _pid: u32) -> Result<String, String> {
        Err("process architecture unavailable".into())
    }
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostPreparation {
    pub(crate) boot_args: String,
    pub(crate) operator_steps: String,
}
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HostPreparationStatus {
    pub(crate) host: HostFacts,
    pub(crate) required_boot_args: String,
    pub(crate) readiness: Eligibility,
    pub(crate) operator_steps: String,
}

const RESEARCH_BOOT_ARGS: [&str; 3] =
    ["amfi_get_out_of_my_way=0x1", "-arm64e_preview_abi", "tss_should_crash=0"];
const OPERATOR_STEPS: &str = "To disable SIP: shut down the Mac; press and hold the power button until startup options appear; choose Options > Continue to enter macOS Recovery; from Utilities open Terminal; run `csrutil disable`; reboot normally. Boot-arg changes also require a reboot. To re-enable SIP later, return to Recovery, run `csrutil enable`, and reboot.";
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_pidpath(pid: libc::pid_t, buffer: *mut libc::c_void, size: u32) -> libc::c_int;
    fn proc_pidinfo(
        pid: libc::pid_t,
        flavor: libc::c_int,
        arg: u64,
        buffer: *mut libc::c_void,
        size: libc::c_int,
    ) -> libc::c_int;
}

pub(crate) struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandResult, String> {
        let result = Command::new(program).args(args).output().map_err(|e| e.to_string())?;
        Ok(CommandResult {
            success: result.status.success(),
            stdout: String::from_utf8_lossy(&result.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
        })
    }

    fn process_signing_status(&mut self, pid: u32) -> Result<u32, String> {
        let mut status: u32 = 0;
        csops_read(pid, 0, (&raw mut status).cast(), std::mem::size_of_val(&status))?;
        Ok(status)
    }

    fn process_cdhash(&mut self, pid: u32) -> Result<[u8; 20], String> {
        let mut hash = [0_u8; 20];
        csops_read(pid, 5, hash.as_mut_ptr().cast(), hash.len())?;
        Ok(hash)
    }

    fn process_executable(&mut self, pid: u32) -> Result<String, String> {
        let pid = i32::try_from(pid).map_err(|e| e.to_string())?;
        let mut buffer = [0_u8; 4096];
        // SAFETY: libproc writes at most the supplied buffer size; only the returned bytes are read.
        let count = unsafe { proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
        if count <= 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        CStr::from_bytes_until_nul(&buffer)
            .map_err(|e| e.to_string())?
            .to_str()
            .map(str::to_owned)
            .map_err(|e| e.to_string())
    }

    fn process_architecture(&mut self, pid: u32) -> Result<String, String> {
        let pid = i32::try_from(pid).map_err(|e| e.to_string())?;
        let mut architecture = [0_i32; 2];
        // SAFETY: PROC_PIDARCHINFO writes two i32 values to the correctly sized live buffer.
        let read = unsafe { proc_pidinfo(pid, 19, 0, architecture.as_mut_ptr().cast(), 8) };
        if read != 8 {
            return Err(format!(
                "proc_pidinfo(PROC_PIDARCHINFO) returned {read}: {}",
                std::io::Error::last_os_error()
            ));
        }
        decode_process_architecture(architecture[0], architecture[1])
            .ok_or_else(|| format!("unsupported process CPU type/subtype: {architecture:?}"))
    }
}

fn csops_read(
    pid: u32,
    operation: u32,
    buffer: *mut libc::c_void,
    size: usize,
) -> Result<(), String> {
    type CsOps = unsafe extern "C" fn(libc::pid_t, u32, *mut libc::c_void, usize) -> libc::c_int;
    static CSOPS: std::sync::LazyLock<Option<CsOps>> = std::sync::LazyLock::new(|| {
        // SAFETY: csops has the XNU ABI; dlsym returns a function pointer or null.
        unsafe {
            let symbol = libc::dlsym(libc::RTLD_DEFAULT, c"csops".as_ptr());
            (!symbol.is_null()).then(|| std::mem::transmute::<*mut libc::c_void, CsOps>(symbol))
        }
    });
    let pid = i32::try_from(pid).map_err(|_| "PID outside pid_t range".to_owned())?;
    let query = (*CSOPS).ok_or_else(|| "csops symbol unavailable".to_owned())?;
    // SAFETY: query's ABI, PID, operation, buffer and length are validated by the caller.
    if unsafe { query(pid, operation, buffer, size) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(())
}

pub(crate) fn decode_process_architecture(cpu_type: i32, cpu_subtype: i32) -> Option<String> {
    let subtype = cpu_subtype & 0x00ff_ffff;
    match (cpu_type, subtype) {
        (0x0100_000c, 0 | 1) => Some("arm64".into()),
        (0x0100_000c, 2) => Some("arm64e".into()),
        (0x0100_000c, 12) => Some("arm64e.x1".into()),
        (0x0100_0007, _) => Some("x86_64".into()),
        _ => None,
    }
}

pub(crate) fn add_research_boot_args(args: &str) -> String {
    let mut tokens: Vec<&str> = args.split_whitespace().collect();
    for arg in RESEARCH_BOOT_ARGS {
        if !tokens.contains(&arg) {
            tokens.push(arg);
        }
    }
    tokens.join(" ")
}

fn read_configured_boot_args<R: CommandRunner>(runner: &mut R) -> Result<String, String> {
    let output = runner.run("/usr/sbin/nvram", &["boot-args"])?;
    if output.success {
        let text = output.stdout.trim();
        let args = text
            .strip_prefix("boot-args")
            .ok_or_else(|| format!("unexpected nvram response: {text}"))?;
        if !args.chars().next().is_none_or(char::is_whitespace) {
            return Err(format!("unexpected nvram response: {text}"));
        }
        return Ok(args.trim().to_owned());
    }
    if output.stderr.contains("data was not found") {
        return Ok(String::new());
    }
    Err(format!("cannot read boot-args: {}", output.stderr.trim()))
}

pub(crate) fn prepare_host_status<R: CommandRunner>(runner: &mut R) -> HostPreparationStatus {
    let host = host_status(runner);
    let required_boot_args = host
        .configured_boot_args
        .value
        .as_ref()
        .map(|args| add_research_boot_args(&args.join(" ")))
        .unwrap_or_else(|| "unavailable: cannot plan without readable nvram boot-args".into());
    let active =
        host.active_boot_args.value.as_ref().map(|args| {
            RESEARCH_BOOT_ARGS.iter().all(|arg| args.iter().any(|actual| actual == arg))
        });
    let readiness =
        if [host.sip_disabled.value, host.amfi_disabled.value, active].contains(&Some(false)) {
            Eligibility::Blocked
        } else if [host.sip_disabled.value, host.amfi_disabled.value, active].contains(&None) {
            Eligibility::Unknown
        } else {
            Eligibility::Eligible
        };
    HostPreparationStatus {
        host,
        required_boot_args,
        readiness,
        operator_steps: OPERATOR_STEPS.into(),
    }
}

/// The nvram variable that records exactly which research tokens this host's `prepare-host` added,
/// so `--revert` removes only those and leaves a pre-existing argument (an operator-set
/// `amfi_get_out_of_my_way=0x1`, for instance) in place.
const RECON_ADDED_VARIABLE: &str = "recon-added-boot-args";

/// The exact research tokens this host's `prepare-host` added, or `None` when no record exists.
///
/// A present-but-empty record (prepare-host ran when every research token was already set) is
/// `Some(vec![])` and means revert removes nothing; an absent record is `None` and means revert
/// must not touch the operator's configuration at all, because Recon has no evidence it set it.
fn read_recon_added<R: CommandRunner>(runner: &mut R) -> Option<Vec<String>> {
    let output = runner.run("/usr/sbin/nvram", &[RECON_ADDED_VARIABLE]).ok()?;
    if !output.success {
        return None;
    }
    let rest = output.stdout.trim().strip_prefix(RECON_ADDED_VARIABLE)?;
    Some(rest.split_whitespace().map(str::to_owned).collect())
}

pub(crate) fn prepare_host<R: CommandRunner>(runner: &mut R) -> Result<HostPreparation, String> {
    let prior = read_configured_boot_args(runner)?;
    let boot_args = add_research_boot_args(&prior);

    // Record what this host's first prepare-host introduced, before changing boot-args, so revert is
    // always precise. A pre-existing record is kept untouched: re-running prepare-host must not
    // relabel an operator argument as one Recon added. If recording fails, abort before weakening
    // the host, so revert can never fall back to removing an argument Recon did not set.
    if read_recon_added(runner).is_none() {
        let prior_tokens: Vec<&str> = prior.split_whitespace().collect();
        let added: Vec<&str> =
            RESEARCH_BOOT_ARGS.iter().copied().filter(|arg| !prior_tokens.contains(arg)).collect();
        let record = format!("{RECON_ADDED_VARIABLE}={}", added.join(" "));
        let recorded = runner.run("/usr/bin/sudo", &["/usr/sbin/nvram", &record])?;
        if !recorded.success {
            return Err(format!(
                "cannot record the research boot-arg delta, so preparation was not applied: {}",
                recorded.stderr.trim()
            ));
        }
    }

    let result =
        runner.run("/usr/bin/sudo", &["/usr/sbin/nvram", &format!("boot-args={boot_args}")])?;
    if !result.success {
        return Err(format!("cannot set research boot-args: {}", result.stderr.trim()));
    }
    Ok(HostPreparation { boot_args, operator_steps: OPERATOR_STEPS.into() })
}

pub(crate) fn revert_host<R: CommandRunner>(runner: &mut R) -> Result<HostPreparation, String> {
    let current = read_configured_boot_args(runner)?;
    // Without a record of what Recon added, do nothing: removing arguments Recon cannot prove it set
    // would clobber the operator's configuration. Recon never guesses here.
    let Some(recorded) = read_recon_added(runner) else {
        return Ok(HostPreparation {
            boot_args: current,
            operator_steps: "No Recon research boot-arg record was found, so nothing was reverted.                              Remove any research boot-args manually if they are no longer wanted."
                .into(),
        });
    };

    let to_remove: Vec<&str> = recorded.iter().map(String::as_str).collect();
    let boot_args = current
        .split_whitespace()
        .filter(|arg| !to_remove.contains(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let removed_any = current.split_whitespace().any(|arg| to_remove.contains(&arg));

    if removed_any {
        let result = if boot_args.is_empty() {
            runner.run("/usr/bin/sudo", &["/usr/sbin/nvram", "-d", "boot-args"])?
        } else {
            runner.run("/usr/bin/sudo", &["/usr/sbin/nvram", &format!("boot-args={boot_args}")])?
        };
        if !result.success {
            return Err(format!("cannot revert research boot-args: {}", result.stderr.trim()));
        }
    }
    // Clear the delta record whether or not a boot-arg change was needed, so the host returns to an
    // unrecorded state. Best-effort: the boot-args are already reverted.
    let _ = runner.run("/usr/bin/sudo", &["/usr/sbin/nvram", "-d", RECON_ADDED_VARIABLE]);
    Ok(HostPreparation { boot_args, operator_steps: "Reboot to apply the reverted boot-args. To re-enable SIP: shut down, enter macOS Recovery by holding the power button until startup options appear; choose Options > Continue; open Utilities > Terminal; run `csrutil enable`; reboot.".into() })
}

fn require(
    decision: &mut ModeDecision,
    fact: &Fact<bool>,
    predicate: bool,
    label: &str,
    remedy: &str,
) {
    decision.evidence_sources.push(fact.source.clone());
    match fact.value {
        Some(value) if value == predicate => {}
        Some(_) => {
            decision.status = Eligibility::Blocked;
            decision.reasons.push(format!("{label}: {remedy}"));
        }
        None => {
            if decision.status != Eligibility::Blocked {
                decision.status = Eligibility::Unknown;
            }
            decision.reasons.push(format!(
                "{label} is unknown ({}: {})",
                fact.source,
                fact.detail.as_deref().unwrap_or("unavailable")
            ));
        }
    }
}

fn reject(decision: &mut ModeDecision, reason: &str) {
    decision.status = Eligibility::Blocked;
    decision.reasons.push(reason.into());
}

fn supported_architecture(decision: &mut ModeDecision, architecture: &Fact<String>, label: &str) {
    decision.evidence_sources.push(architecture.source.clone());
    match architecture.value.as_deref() {
        Some("arm64" | "arm64e" | "arm64e.x1") => {}
        Some(_) => {
            reject(decision, &format!("{label} must be arm64 or arm64e (including arm64e.x1)"))
        }
        None => {
            if decision.status != Eligibility::Blocked {
                decision.status = Eligibility::Unknown;
            }
            decision
                .reasons
                .push(format!("{label} architecture is unknown ({})", architecture.source));
        }
    }
}

pub(crate) fn evaluate(
    target: &TargetFacts,
    host: &HostFacts,
    helper: &HelperFacts,
) -> Vec<ModeDecision> {
    evaluate_with_authorization(target, host, helper, false)
}

pub(crate) fn evaluate_with_authorization(
    target: &TargetFacts,
    host: &HostFacts,
    helper: &HelperFacts,
    allow_authorization: bool,
) -> Vec<ModeDecision> {
    let mut decisions = Vec::with_capacity(6);
    for profile in [AccessProfile::Standard, AccessProfile::Research] {
        for mode in [AccessMode::Existing, AccessMode::Launch, AccessMode::Attach] {
            let mut decision = ModeDecision {
                mode,
                profile,
                status: Eligibility::Eligible,
                reasons: vec![],
                evidence_sources: vec![],
            };
            decision.evidence_sources.push(host.os_version.source.clone());
            match host.os_version.value.as_deref() {
                Some(version) if version.split('.').next() == Some("27") => {}
                Some(_) => reject(&mut decision, "inspection requires macOS 27"),
                None => {
                    decision.status = Eligibility::Unknown;
                    decision.reasons.push("macOS version is unknown".into());
                }
            }
            supported_architecture(&mut decision, &host.architecture, "host");
            supported_architecture(&mut decision, &target.architecture, "target");
            if mode == AccessMode::Launch {
                if !target.running_pids.is_empty() || target.pid.is_some() {
                    reject(
                        &mut decision,
                        "launch requires a non-running target; no automatic replacement",
                    );
                }
                require(
                    &mut decision,
                    &helper.local_available,
                    true,
                    "local helper",
                    "helper is unavailable",
                );
                require(
                    &mut decision,
                    &helper.local_compatible,
                    true,
                    "local helper",
                    "helper build/protocol mismatch",
                );
                decision.evidence_sources.extend([
                    target.hardened_runtime.source.clone(),
                    target.allow_dyld_environment_variables.source.clone(),
                ]);
                match (target.hardened_runtime.value, target.allow_dyld_environment_variables.value)
                {
                    (Some(false), _) | (Some(true), Some(true)) => {}
                    (Some(true), Some(false)) => reject(
                        &mut decision,
                        "hardened runtime rejects DYLD injection without allow-dyld-environment-variables; attach explicitly instead",
                    ),
                    _ if decision.status != Eligibility::Blocked => {
                        decision.status = Eligibility::Unknown;
                        decision
                            .reasons
                            .push("launch DYLD injection eligibility cannot be established".into());
                    }
                    _ => {}
                }
            } else {
                if target.pid.is_none()
                    || !target.pid.is_some_and(|pid| target.running_pids.contains(&pid))
                {
                    reject(&mut decision, "mode requires one selected running process");
                }
                decision.evidence_sources.push(target.process_executable.source.clone());
                if let (Some(actual), Some(expected)) =
                    (&target.process_executable.value, &target.executable_path.value)
                {
                    if !same_path(actual, expected) {
                        reject(
                            &mut decision,
                            "selected PID executable does not match the selected bundle",
                        );
                    }
                } else if decision.status != Eligibility::Blocked {
                    decision.status = Eligibility::Unknown;
                    decision.reasons.push("process executable identity cannot be verified".into());
                }
                if mode == AccessMode::Existing {
                    require(
                        &mut decision,
                        &helper.resident,
                        true,
                        "resident helper",
                        "no helper exists in the selected process",
                    );
                    require(
                        &mut decision,
                        &helper.resident_compatible,
                        true,
                        "resident helper",
                        "helper build/protocol mismatch",
                    );
                } else {
                    require(
                        &mut decision,
                        &helper.local_available,
                        true,
                        "local helper",
                        "helper is unavailable",
                    );
                    require(
                        &mut decision,
                        &helper.local_compatible,
                        true,
                        "local helper",
                        "helper build/protocol mismatch",
                    );
                    if !allow_authorization {
                        require(
                            &mut decision,
                            &helper.attach_without_dialog,
                            true,
                            "debugger authorization",
                            "requires explicit --allow-authorization before a possible dialog",
                        );
                    }
                }
            }
            let system_bundle = target.bundle_path.starts_with("/System/");
            let first_party = match target.apple_signed.value {
                Some(true) => Some(true),
                Some(false) if system_bundle => Some(true),
                Some(false) => Some(false),
                None if system_bundle => Some(true),
                None => None,
            };
            decision.evidence_sources.extend([
                target.apple_signed.source.clone(),
                target.platform_binary.source.clone(),
            ]);
            let requires_research = if first_party == Some(true) {
                Some(true)
            } else {
                target.get_task_allow.value.map(|cooperative| !cooperative)
            };
            match requires_research {
                Some(true) if profile == AccessProfile::Standard => reject(
                    &mut decision,
                    "target requires the research profile and recon ui session prepare-host",
                ),
                Some(true) => {
                    require(
                        &mut decision,
                        &host.sip_disabled,
                        true,
                        "SIP",
                        "disable SIP in Recovery; recon ui session prepare-host prints operator steps",
                    );
                    require(
                        &mut decision,
                        &host.amfi_disabled,
                        true,
                        "AMFI",
                        "recon ui session prepare-host is required, then reboot",
                    );
                    decision.evidence_sources.push(host.active_boot_args.source.clone());
                    match host.active_boot_args.value.as_ref() {
                        Some(args)
                            if RESEARCH_BOOT_ARGS
                                .iter()
                                .all(|required| args.iter().any(|actual| actual == required)) => {}
                        Some(_) => reject(
                            &mut decision,
                            "research boot-args are not active; recon ui session prepare-host then reboot",
                        ),
                        None if decision.status != Eligibility::Blocked => {
                            decision.status = Eligibility::Unknown;
                            decision
                                .reasons
                                .push("active research boot-args could not be read".into());
                        }
                        None => {}
                    }
                    if first_party == Some(true) {
                        require(
                            &mut decision,
                            &helper.inspector_thread_set_state,
                            true,
                            "first-party loader signing",
                            "Recon inspector requires com.apple.private.thread-set-state",
                        );
                    }
                }
                None if decision.status != Eligibility::Blocked => {
                    decision.status = Eligibility::Unknown;
                    decision.reasons.push(
                        "target cooperation or first-party protection could not be established"
                            .into(),
                    );
                }
                _ => {}
            }
            decisions.push(decision);
        }
    }
    decisions
}

fn same_path(actual: &str, expected: &str) -> bool {
    actual == expected
        || Path::new(actual)
            .canonicalize()
            .ok()
            .zip(Path::new(expected).canonicalize().ok())
            .is_some_and(|(a, b)| a == b)
}

fn read_output<R: CommandRunner>(
    runner: &mut R,
    program: &str,
    args: &[&str],
) -> Result<CommandResult, String> {
    let output = runner.run(program, args)?;
    if !output.success {
        return Err(format!("{program}: {}", output.stderr.trim()));
    }
    Ok(output)
}

fn read_text<R: CommandRunner>(runner: &mut R, program: &str, args: &[&str]) -> Fact<String> {
    let source = format!("{program} {}", args.join(" "));
    match read_output(runner, program, args) {
        Ok(output) if !output.stdout.trim().is_empty() => {
            Fact::known(output.stdout.trim().into(), &source)
        }
        Ok(_) => Fact::unknown(&source, "empty response"),
        Err(error) => Fact::unknown(&source, &error),
    }
}

pub(crate) fn host_status<R: CommandRunner>(runner: &mut R) -> HostFacts {
    let os_version = read_text(runner, "/usr/bin/sw_vers", &["-productVersion"]);
    let architecture = read_text(runner, "/usr/bin/uname", &["-m"]);
    let sip = read_text(runner, "/usr/bin/csrutil", &["status"]);
    let sip_disabled = match sip.value.as_deref() {
        Some(status) if status.contains("System Integrity Protection status: disabled") => {
            Fact::known(true, &sip.source)
        }
        Some(status) if status.contains("System Integrity Protection status: enabled") => {
            Fact::known(false, &sip.source)
        }
        _ => Fact::unknown(&sip.source, sip.detail.as_deref().unwrap_or("unrecognized SIP status")),
    };
    let active = runner.run("/usr/sbin/sysctl", &["-n", "kern.bootargs"]);
    let active_boot_args = match active {
        Ok(output) if output.success => Fact::known(
            output.stdout.split_whitespace().map(str::to_owned).collect::<Vec<_>>(),
            "sysctl kern.bootargs (current boot)",
        ),
        Ok(output) => Fact::unknown("sysctl kern.bootargs (current boot)", &output.stderr),
        Err(error) => Fact::unknown("sysctl kern.bootargs (current boot)", &error),
    };
    let amfi_disabled = match active_boot_args.value.as_ref() {
        Some(args) => Fact::known(
            args.iter().any(|arg| arg == RESEARCH_BOOT_ARGS[0]),
            "sysctl kern.bootargs (current boot, AMFI boot setting)",
        ),
        None => Fact::unknown(
            "sysctl kern.bootargs (current boot, AMFI boot setting)",
            active_boot_args.detail.as_deref().unwrap_or("unavailable"),
        ),
    };
    let configured_boot_args = match read_configured_boot_args(runner) {
        Ok(args) => Fact::known(
            args.split_whitespace().map(str::to_owned).collect(),
            "nvram boot-args (next boot)",
        ),
        Err(error) => Fact::unknown("nvram boot-args (next boot)", &error),
    };
    HostFacts {
        os_version,
        architecture,
        sip_disabled,
        amfi_disabled,
        active_boot_args,
        configured_boot_args,
    }
}

fn selected_cdhash(display: &str) -> Option<[u8; 20]> {
    let hex = display.lines().find_map(|line| line.strip_prefix("CDHash="))?.trim();
    if hex.len() != 40 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut hash = [0_u8; 20];
    for (index, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(hash)
}

fn signing_facts<R: CommandRunner>(
    runner: &mut R,
    executable: &Fact<String>,
    target: &mut TargetFacts,
) -> Option<[u8; 20]> {
    let path = executable.value.as_deref()?;
    let arch = target.architecture.value.as_deref();
    let source = format!("codesign --display --verbose=4 {path} (on-disk executable)");
    let display = match arch {
        Some(arch) => runner
            .run("/usr/bin/codesign", &["--display", "--architecture", arch, "--verbose=4", path]),
        None => runner.run("/usr/bin/codesign", &["--display", "--verbose=4", path]),
    };
    let mut disk_hash = None;
    match display {
        Ok(output) if output.success => {
            target.signed = Fact::known(true, &source);
            let display = format!("{}\n{}", output.stderr, output.stdout);
            disk_hash = selected_cdhash(&display);
            let mut authorities =
                display.lines().filter_map(|line| line.strip_prefix("Authority="));
            target.apple_signed = match authorities.next() {
                Some(first) => Fact::known(
                    first.starts_with("Apple ")
                        || first == "Apple Root CA"
                        || authorities
                            .any(|name| name.starts_with("Apple ") || name == "Apple Root CA"),
                    &source,
                ),
                None => Fact::unknown(&source, "signing authority not reported"),
            };
            let flags =
                display.lines().find_map(|line| line.split("flags=0x").nth(1)).and_then(|hex| {
                    let width = hex.bytes().take_while(u8::is_ascii_hexdigit).count();
                    u32::from_str_radix(&hex[..width], 16).ok()
                });
            if let Some(flags) = flags {
                target.hardened_runtime = Fact::known(flags & 0x10000 != 0, &source);
                target.library_validation = Fact::known(flags & 0x2000 != 0, &source);
                target.platform_binary = Fact::known(flags & 0x0400_0000 != 0, &source);
            }
            let ent_source =
                format!("codesign --display --entitlements - --xml {path} (on-disk executable)");
            let entitlements = match arch {
                Some(arch) => read_output(
                    runner,
                    "/usr/bin/codesign",
                    &["--display", "--architecture", arch, "--entitlements", "-", "--xml", path],
                ),
                None => read_output(
                    runner,
                    "/usr/bin/codesign",
                    &["--display", "--entitlements", "-", "--xml", path],
                ),
            };
            match entitlements {
                Ok(entitlements) if entitlements.stdout.is_empty() => {
                    target.get_task_allow = Fact::known(false, &ent_source);
                    target.allow_dyld_environment_variables = Fact::known(false, &ent_source);
                }
                Ok(entitlements) => {
                    match plist::Value::from_reader_xml(entitlements.stdout.as_bytes()) {
                        Ok(plist::Value::Dictionary(dictionary)) => {
                            let entitlement = |key: &str| match dictionary.get(key) {
                                None => Fact::known(false, &ent_source),
                                Some(value) => value
                                    .as_boolean()
                                    .map(|value| Fact::known(value, &ent_source))
                                    .unwrap_or_else(|| {
                                        Fact::unknown(&ent_source, "entitlement is not a Boolean")
                                    }),
                            };
                            target.get_task_allow =
                                entitlement("com.apple.security.get-task-allow");
                            target.allow_dyld_environment_variables = entitlement(
                                "com.apple.security.cs.allow-dyld-environment-variables",
                            );
                        }
                        _ => {
                            target.get_task_allow = Fact::unknown(
                                &ent_source,
                                "entitlements are not a readable XML dictionary",
                            );
                            target.allow_dyld_environment_variables = Fact::unknown(
                                &ent_source,
                                "entitlements are not a readable XML dictionary",
                            );
                        }
                    }
                }
                Err(error) => {
                    target.get_task_allow = Fact::unknown(&ent_source, &error);
                    target.allow_dyld_environment_variables = Fact::unknown(&ent_source, &error);
                }
            }
        }
        Ok(output) if output.stderr.contains("code object is not signed at all") => {
            target.signed = Fact::known(false, &source);
        }
        Ok(output) => target.signed = Fact::unknown(&source, &output.stderr),
        Err(error) => target.signed = Fact::unknown(&source, &error),
    }
    disk_hash
}

pub(crate) fn collect_preflight<R: CommandRunner>(
    runner: &mut R,
    selection: TargetSelection,
    helper: HelperFacts,
) -> PreflightReport {
    let host = host_status(runner);
    let pid = selection
        .pid
        .or_else(|| (selection.running_pids.len() == 1).then(|| selection.running_pids[0]));
    let executable_path = selection
        .executable_path
        .as_ref()
        .map(|path| Fact::known(path.display().to_string(), "selected bundle Contents/Info.plist"))
        .unwrap_or_else(|| {
            Fact::unknown("selected bundle Contents/Info.plist", "no readable executable path")
        });
    fn unknown<T>(field: &str) -> Fact<T> {
        Fact::unknown(field, "not yet observed")
    }
    let mut target = TargetFacts {
        bundle_path: selection.bundle_path,
        bundle_id: selection.bundle_id,
        executable_path: executable_path.clone(),
        pid,
        running_pids: selection.running_pids,
        process_executable: unknown("proc_pidpath (live process executable)"),
        architecture: unknown("proc_pidinfo PROC_PIDARCHINFO (live process architecture)"),
        executable_architectures: unknown("lipo -archs (on-disk executable)"),
        signed: unknown("codesign (on-disk executable)"),
        apple_signed: unknown("codesign authority (on-disk executable)"),
        get_task_allow: unknown("codesign entitlements (on-disk executable)"),
        allow_dyld_environment_variables: unknown("codesign entitlements (on-disk executable)"),
        hardened_runtime: unknown("codesign flags (on-disk executable)"),
        library_validation: unknown("codesign flags (on-disk executable)"),
        platform_binary: unknown("codesign flags (on-disk executable)"),
    };
    if let Some(pid) = target.pid {
        target.process_executable = match runner.process_executable(pid) {
            Ok(path) => Fact::known(path, "proc_pidpath (live process executable)"),
            Err(error) => Fact::unknown("proc_pidpath (live process executable)", &error),
        };
        target.architecture = match runner.process_architecture(pid) {
            Ok(arch) => {
                Fact::known(arch, "proc_pidinfo PROC_PIDARCHINFO (live process architecture)")
            }
            Err(error) => {
                Fact::unknown("proc_pidinfo PROC_PIDARCHINFO (live process architecture)", &error)
            }
        };
    }
    let disk_hash = if let Some(path) = executable_path.value.as_deref() {
        target.executable_architectures = read_text(runner, "/usr/bin/lipo", &["-archs", path])
            .map(|arches| arches.split_whitespace().map(str::to_owned).collect());
        if target.pid.is_none() {
            target.architecture = match target.executable_architectures.value.as_ref() {
                Some(architectures) if architectures.len() == 1 => Fact::known(
                    architectures[0].clone(),
                    "lipo -archs (single on-disk slice; launch prediction)",
                ),
                _ => Fact::unknown(
                    "lipo -archs (on-disk executable)",
                    "running architecture cannot be determined before launch",
                ),
            };
        }
        signing_facts(runner, &executable_path, &mut target)
    } else {
        None
    };
    if let Some(pid) = target.pid {
        let source = format!("csops PID {pid} (live kernel signing status)");
        match runner.process_signing_status(pid) {
            Ok(flags) => {
                target.signed = Fact::known(flags & 0x2000_0000 != 0, &source);
                target.get_task_allow = Fact::known(flags & 0x4 != 0, &source);
                target.hardened_runtime = Fact::known(flags & 0x10000 != 0, &source);
                target.library_validation = Fact::known(flags & 0x2000 != 0, &source);
                target.platform_binary = Fact::known(flags & 0x0400_0000 != 0, &source);
            }
            Err(error) => {
                target.signed = Fact::unknown(&source, &error);
                target.get_task_allow = Fact::unknown(&source, &error);
                target.hardened_runtime = Fact::unknown(&source, &error);
                target.library_validation = Fact::unknown(&source, &error);
                target.platform_binary = Fact::unknown(&source, &error);
            }
        }
        let hash_source =
            format!("codesign selected CDHash + csops PID {pid} CDHash (live process binding)");
        match (disk_hash, runner.process_cdhash(pid)) {
            (Some(disk), Ok(live)) if disk == live => {
                target.apple_signed.source = hash_source.clone();
                target.allow_dyld_environment_variables.source = hash_source;
            }
            (Some(_), Ok(_)) => {
                target.apple_signed = Fact::unknown(
                    &hash_source,
                    "on-disk signature does not match the running process",
                );
                target.allow_dyld_environment_variables = Fact::unknown(
                    &hash_source,
                    "on-disk entitlements do not match the running process",
                );
            }
            (_, Err(error)) => {
                target.apple_signed = Fact::unknown(&hash_source, &error);
                target.allow_dyld_environment_variables = Fact::unknown(&hash_source, &error);
            }
            (None, _) => {
                target.apple_signed =
                    Fact::unknown(&hash_source, "on-disk CodeDirectory hash is unavailable");
                target.allow_dyld_environment_variables =
                    Fact::unknown(&hash_source, "on-disk CodeDirectory hash is unavailable");
            }
        }
    }
    let decisions = evaluate(&target, &host, &helper);
    PreflightReport { target, host, helper, decisions }
}

impl<T> Fact<T> {
    fn map<U>(self, transform: impl FnOnce(T) -> U) -> Fact<U> {
        Fact { value: self.value.map(transform), source: self.source, detail: self.detail }
    }
}
