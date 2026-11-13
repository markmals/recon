#!/usr/bin/env bash
# Applies the experiment's VM settings to VirtualBuddy's Config.plist after installation
# and before the first boot:
#
#   - no VirtualBuddyGuest disk, clipboard sharing, shared folders, or audio devices
#     (the guest gets no host microphone or speaker);
#   - a 2560x1600 @ 218 ppi display (1280x800 points), which fits the host screen
#     closely enough for screenshots to stay legible.
#
# VirtualBuddy's first-boot account provisioning is left off. Its password fields turn
# on secure event input, which Recon's `ui key` refuses to type into, and writing the
# keychain item it reads from outside VirtualBuddy makes macOS ask for the host login
# password. The account is created through Setup Assistant in the VM window instead.
#
# VirtualBuddy rewrites the configuration of an open VM, so it must not be running.
#
# usage: configure-vm.sh
set -euo pipefail
source "$(dirname "$0")/env.sh"

if pgrep -x VirtualBuddy >/dev/null; then
    echo "configure-vm: quit VirtualBuddy first; it rewrites the configuration of open VMs" >&2
    exit 1
fi

config="$VM_BUNDLE/.vbdata/Config.plist"
plutil -replace guestAdditionsEnabled -bool NO "$config"
plutil -replace clipboardSharingEnabled -bool NO "$config"
plutil -replace sharedFolders -json '[]' "$config"
plutil -replace hardware.soundDevices -json '[]' "$config"
plutil -replace hardware.displayDevices.0.width -integer 2560 "$config"
plutil -replace hardware.displayDevices.0.height -integer 1600 "$config"
plutil -replace hardware.displayDevices.0.pixelsPerInch -integer 218 "$config"
plutil -replace hardware.displayDevices.0.name -string "2560x1600@218" "$config"
plutil -replace provisioningEnabled -bool NO "$config"
plutil -remove provisioning "$config" 2>/dev/null || true

plutil -convert json -o - "$config" | jq '{guestAdditionsEnabled, clipboardSharingEnabled,
    sharedFolders, provisioningEnabled, display: .hardware.displayDevices[0].name,
    sound: .hardware.soundDevices}'
