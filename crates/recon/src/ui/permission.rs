//! The privacy permissions `recon ui` needs, and how a missing one is explained.

use std::path::Path;

/// A macOS privacy permission that `recon` itself must hold.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Permission {
    ScreenRecording,
    Accessibility,
}

impl Permission {
    fn name(self) -> &'static str {
        match self {
            Permission::ScreenRecording => "Screen Recording",
            Permission::Accessibility => "Accessibility",
        }
    }

    fn settings_location(self) -> &'static str {
        match self {
            Permission::ScreenRecording => "Privacy & Security → Screen & System Audio Recording",
            Permission::Accessibility => "Privacy & Security → Accessibility",
        }
    }
}

/// How the running `recon` executable is code-signed, which decides whether macOS keeps a grant
/// when the executable is rebuilt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Signature {
    /// The designated requirement is the code hash itself, so a rebuild forgets every grant.
    AdHoc,
    /// Any other signature, such as Developer ID, whose grants outlive a rebuild. A signature
    /// Recon cannot read is taken to be this kind, so no rebuild warning is given for it.
    Stable,
}

const AD_HOC_SIGNATURE: &str = "\nthis recon is ad-hoc signed, so macOS forgets the grant \
    whenever this executable is rebuilt; signing it with a Developer ID Application identity keeps \
    the grant across rebuilds";

/// Explains which permissions are missing, for which executable, and where to grant them.
pub fn missing_permissions_message(
    missing: &[Permission],
    executable: &Path,
    signature: Signature,
) -> String {
    let noun = if missing.len() == 1 { "permission" } else { "permissions" };
    let grants: String = missing
        .iter()
        .map(|permission| {
            let location = permission.settings_location();
            format!("\n  {}: System Settings → {location}", permission.name())
        })
        .collect();
    let rebuilds = match signature {
        Signature::AdHoc => AD_HOC_SIGNATURE,
        Signature::Stable => "",
    };
    format!("missing {noun} for {}:{grants}{rebuilds}", executable.display())
}
