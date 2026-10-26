//! What Recon is, and what it can do here: the version and path of the running executable, the
//! host it was built for, and — on a Mac — its signature, its privacy identity, the permissions
//! it holds, the private interfaces it depends on, and whether its application discovery works.
//!
//! Everything here reads state. Nothing in this report asks macOS to request a permission, and
//! nothing it does can put a dialog on a screen.

use std::env;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::Serialize;

use crate::error::Failure;
use crate::ui::permission::Permission;

/// The execution context of one `recon` installation.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Context {
    pub recon: ReconFacts,
    pub host: HostFacts,
    /// The macOS-only facts, and null on every other host: knowing that this host supports
    /// nothing is the answer rather than a failure.
    pub macos: Option<MacosFacts>,
}

/// The running executable.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReconFacts {
    /// The crate version of this build.
    pub version: String,
    /// The full commit hash this build was made from, or null when it was built outside a Git
    /// checkout.
    pub commit: Option<String>,
    /// The canonicalized absolute path of the running executable, which is the path a macOS
    /// privacy grant is recorded against.
    pub executable_path: String,
}

/// The machine Recon is running on, as the build knows it.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HostFacts {
    /// The operating system this build targets.
    pub os: String,
    /// The macOS product version as `major.minor.patch`, read from the structured API rather
    /// than the version string Apple documents as unsuitable for parsing, and null on other
    /// hosts.
    pub version: Option<String>,
    /// The processor architecture this build targets.
    pub architecture: String,
}

/// What this Mac says about Recon.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MacosFacts {
    pub code_signing: CodeSigningFacts,
    pub responsibility: ResponsibilityFacts,
    /// One entry per permission Recon can need, in a fixed order.
    pub permissions: Vec<PermissionState>,
    /// One entry per private interface in a fixed order. The required code-signing-query
    /// must be available; other absent symbols remain reportable context facts.
    pub interfaces: Vec<InterfaceState>,
    pub application_discovery: ApplicationDiscovery,
}

/// Running-process signing facts, bound to its kernel hash without a validity or trust check.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodeSigningFacts {
    /// Whether the kernel reports a signature, even if that signature is invalid or untrusted.
    pub signed: bool,
    /// The code-signing identifier, or null when the code is unsigned.
    pub identifier: Option<String>,
    /// The signing team, or null when the signature names none.
    pub team_identifier: Option<String>,
    /// Whether the signature is ad-hoc. An ad-hoc designated requirement is the code hash
    /// itself, so macOS forgets every grant when the executable is rebuilt.
    pub ad_hoc: bool,
}

/// Whether macOS attributes Recon's privacy requests to Recon itself.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResponsibilityFacts {
    pub responsible_for_self: bool,
    /// Why Recon is not responsible for itself, and null when it is.
    pub detail: Option<String>,
}

/// One privacy permission, as a preflight check answered it.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionState {
    pub permission: Permission,
    /// Whether macOS has granted the permission, and null — never false — whenever Recon is not
    /// responsible for itself, because a preflight run under the launcher's identity answers a
    /// different question.
    pub granted: Option<bool>,
    /// Where in System Settings a person grants it.
    pub settings_pane: String,
}

/// One private interface Recon depends on, and whether this macOS still has it.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InterfaceState {
    pub interface: String,
    /// The symbol whose presence decides availability.
    pub symbol: String,
    /// Whether the symbol resolves on this macOS. Looking a symbol up does not call it.
    pub available: bool,
}

/// Whether Recon can discover installed applications.
///
/// This reports whether Recon's own discovery path works, not whether Spotlight is enabled
/// system-wide: macOS documents no interface that answers the latter. It is measured by running
/// the same query `recon app list` runs and counting what comes back.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplicationDiscovery {
    pub mechanism: DiscoveryMechanism,
    /// False when the query cannot run, exits unsuccessfully, or reports nothing, which are the
    /// three conditions under which `recon app list` fails.
    pub available: bool,
    /// How many application bundles the query reported, or null when it did not run.
    pub indexed_applications: Option<usize>,
}

/// How Recon discovers installed applications.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DiscoveryMechanism {
    Spotlight,
}

/// Reports this installation's version, identity, permissions, and host capabilities.
pub fn report() -> Result<Context, Failure> {
    // Becoming responsible for Recon's own privacy requests comes first, because doing so may
    // re-execute Recon: anything read before that would be discarded with the parent process.
    let macos = macos()?;
    let executable = executable_path()?;
    let executable_path = executable
        .to_str()
        .ok_or_else(|| {
            Failure::internal(format!(
                "the running recon executable has a path that is not UTF-8: {}",
                executable.display()
            ))
        })?
        .to_owned();

    Ok(Context {
        recon: ReconFacts {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            commit: option_env!("RECON_COMMIT")
                .filter(|commit| !commit.is_empty())
                .map(str::to_owned),
            executable_path,
        },
        host: HostFacts {
            os: env::consts::OS.to_owned(),
            version: host_version(),
            architecture: env::consts::ARCH.to_owned(),
        },
        macos,
    })
}

/// The report as the one JSON object `recon context` prints.
pub fn context() -> Result<String, Failure> {
    serde_json::to_string(&report()?)
        .map_err(|error| Failure::internal(format!("cannot encode response: {error}")))
}

/// The canonicalized absolute path of the running executable, which is the path macOS records a
/// privacy grant against.
pub fn executable_path() -> Result<PathBuf, Failure> {
    let executable = env::current_exe().map_err(|error| {
        Failure::internal(format!("cannot find the running recon executable: {error}"))
    })?;
    executable.canonicalize().map_err(|error| {
        Failure::internal(format!(
            "cannot resolve the running recon executable {}: {error}",
            executable.display()
        ))
    })
}

#[cfg(target_os = "macos")]
fn macos() -> Result<Option<MacosFacts>, Failure> {
    crate::macos::macos_facts().map(Some)
}

#[cfg(not(target_os = "macos"))]
fn macos() -> Result<Option<MacosFacts>, Failure> {
    Ok(None)
}

#[cfg(target_os = "macos")]
fn host_version() -> Option<String> {
    Some(crate::macos::product_version())
}

#[cfg(not(target_os = "macos"))]
fn host_version() -> Option<String> {
    None
}
