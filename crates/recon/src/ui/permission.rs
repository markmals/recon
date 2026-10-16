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

const AD_HOC_SIGNATURE: &str = "\nthis recon is ad-hoc signed, so macOS forgets the grant \
    whenever this executable is rebuilt; signing it with a Developer ID Application identity keeps \
    the grant across rebuilds";

/// Explains which permissions are missing, for which executable, and where to grant them.
pub fn missing_permissions_message(
    missing: &[Permission],
    executable: &Path,
    ad_hoc: bool,
) -> String {
    let noun = if missing.len() == 1 { "permission" } else { "permissions" };
    let grants: String = missing
        .iter()
        .map(|permission| {
            let location = permission.settings_location();
            format!("\n  {}: System Settings → {location}", permission.name())
        })
        .collect();
    let signature = if ad_hoc { AD_HOC_SIGNATURE } else { "" };
    format!("missing {noun} for {}:{grants}{signature}", executable.display())
}
