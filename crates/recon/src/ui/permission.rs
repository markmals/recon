//! The privacy permissions `recon` itself must hold, and how a missing one is reported.

use std::path::Path;

use schemars::JsonSchema;
use serde::Serialize;
use serde_json::json;

use crate::error::{ErrorCode, Failure};

/// A macOS privacy permission that `recon` itself must hold.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    ScreenRecording,
    Accessibility,
}

impl Permission {
    /// Every permission Recon can need, in the order `recon context` reports them.
    pub const ALL: [Permission; 2] = [Permission::ScreenRecording, Permission::Accessibility];

    /// The name the schema, the context report, and the error object publish. It is the stable
    /// one: wording can change, this cannot.
    pub fn identifier(self) -> &'static str {
        match self {
            Permission::ScreenRecording => "screen-recording",
            Permission::Accessibility => "accessibility",
        }
    }

    /// The name macOS gives the permission, for the person reading the message.
    pub fn name(self) -> &'static str {
        match self {
            Permission::ScreenRecording => "Screen Recording",
            Permission::Accessibility => "Accessibility",
        }
    }

    /// Where in System Settings a person grants it.
    pub fn settings_pane(self) -> &'static str {
        match self {
            Permission::ScreenRecording => {
                "System Settings → Privacy & Security → Screen & System Audio Recording"
            }
            Permission::Accessibility => "System Settings → Privacy & Security → Accessibility",
        }
    }
}

/// Reports which permissions are missing, which executable macOS records a grant against, and
/// whether that executable's signature survives a rebuild.
///
/// An ad-hoc signature's designated requirement is the code hash itself, so macOS forgets the
/// grant whenever the executable is rebuilt. That is a fact about the binary, so it is a field
/// rather than a sentence appended to the message.
pub fn permission_denied(
    missing: &[Permission],
    executable: &Path,
    ad_hoc_signed: bool,
) -> Failure {
    let noun = if missing.len() == 1 { "permission" } else { "permissions" };
    let names: Vec<&str> = missing.iter().map(|permission| permission.name()).collect();
    let executable = executable.display().to_string();
    let permissions: Vec<_> = missing
        .iter()
        .map(|permission| {
            json!({
                "permission": permission.identifier(),
                "settingsPane": permission.settings_pane(),
            })
        })
        .collect();

    Failure::new(
        ErrorCode::PermissionDenied,
        format!("missing {noun} for {executable}: {}", names.join(", ")),
        json!({
            "permissions": permissions,
            "executablePath": executable,
            "adHocSigned": ad_hoc_signed,
        }),
    )
}
