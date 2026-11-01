//! Which executable a macOS privacy grant belongs to, and what this Mac says about it.
//!
//! Everything here reads state. Nothing asks macOS to request a permission, and nothing it does
//! can put a dialog on a screen.

use std::env;
use std::path::PathBuf;

use recon_core::error::Failure;
use schemars::JsonSchema;
use serde::Serialize;

use crate::permission::Permission;

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

/// What this Mac says about the executable its privacy grants belong to.
#[derive(Clone, Debug)]
pub struct PrivacyFacts {
    pub code_signing: CodeSigningFacts,
    pub responsibility: ResponsibilityFacts,
    /// One entry per permission Recon can need, in a fixed order.
    pub permissions: Vec<PermissionState>,
}

/// The canonicalized absolute path of the running executable, which is the path macOS records a
/// privacy grant against.
///
/// # Errors
///
/// [`recon_core::error::ErrorCode::Internal`] when the executable cannot be found or resolved.
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

/// Takes responsibility for Recon's own privacy requests, then reports the identity that gives
/// it: how this executable is signed, whether the attribution succeeded, and which permissions
/// macOS has granted it.
///
/// Pre-spawn identity failures are reportable; a post-spawn failure owns the final response.
///
/// # Errors
///
/// The failure of a re-execution that already happened, which no report can speak over.
#[cfg(target_os = "macos")]
pub fn facts() -> Result<PrivacyFacts, Failure> {
    use crate::responsibility::{self, ResponsibilityError};

    let responsibility = match responsibility::take_responsibility() {
        Ok(()) => ResponsibilityFacts { responsible_for_self: true, detail: None },
        Err(ResponsibilityError::BeforeSpawn(failure)) => {
            ResponsibilityFacts { responsible_for_self: false, detail: Some(failure.message) }
        }
        Err(ResponsibilityError::AfterSpawn(failure)) => return Err(failure),
    };
    let responsible = responsibility.responsible_for_self;

    Ok(PrivacyFacts {
        code_signing: crate::signing::facts()?,
        responsibility,
        permissions: Permission::ALL
            .into_iter()
            .map(|permission| PermissionState {
                permission,
                // A preflight run under the launcher's identity answers a different question, so
                // an unknown grant is reported as unknown rather than as a denial.
                granted: responsible.then(|| crate::permissions::granted(permission)),
                settings_pane: permission.settings_pane().to_owned(),
            })
            .collect(),
    })
}
