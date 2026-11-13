#!/usr/bin/env bash
# Saves the stopped VM as a named snapshot, or replaces the VM with a fresh copy of one.
#
# A snapshot is an APFS clone of the whole .vbvm bundle: the disk image (with the guest's
# SIP policy, installed tools, Recon build, and privacy grants), the auxiliary storage
# that holds the guest NVRAM and so the boot-args, the machine identity, and
# VirtualBuddy's configuration. Saving and resetting take about a second, and a
# snapshot costs disk space only for the blocks the running copy later changes.
#
# Only one copy is ever in VirtualBuddy's library. Every copy shares the VM's name, MAC
# address, and machine identifier, which the deep links, the DHCP lease, and the guest's
# security policy depend on, so two copies must never run at once.
#
# usage: vm-snapshot.sh list
#        vm-snapshot.sh save NAME      (replaces an existing snapshot of that name)
#        vm-snapshot.sh reset NAME     (discards the VM and all changes since NAME)
set -euo pipefail
source "$(dirname "$0")/env.sh"

snapshots="$VM_WORKDIR/snapshots"
usage="usage: vm-snapshot.sh list | save NAME | reset NAME"

# VirtualBuddy holds the library's bundles open and rewrites the configuration of a VM
# it has loaded, so the bundle is only touched while VirtualBuddy is not running.
require_virtualbuddy_quit() {
    if pgrep -x VirtualBuddy >/dev/null; then
        echo "vm-snapshot: shut the VM down and quit VirtualBuddy first" >&2
        exit 1
    fi
}

snapshot_path() {
    local name=${1:?$usage}
    if [[ ! $name =~ ^[A-Za-z0-9._-]+$ ]]; then
        echo "vm-snapshot: a snapshot name uses only letters, digits, '.', '_' and '-'" >&2
        exit 2
    fi
    printf '%s/%s.vbvm' "$snapshots" "$name"
}

# Clones SOURCE to DESTINATION through a temporary name on the same volume, so an
# interrupted clone never leaves a half-written bundle under the final name.
clone_bundle() {
    local source=$1 destination=$2
    local partial="$snapshots/.partial.vbvm"
    rm -rf "$partial"
    cp -c -R "$source" "$partial"
    rm -rf "$destination"
    mv "$partial" "$destination"
}

action=${1:?$usage}
case "$action" in
    list)
        mkdir -p "$snapshots"
        for bundle in "$snapshots"/*.vbvm; do
            [[ -e $bundle ]] || continue
            printf '%-32s saved %s\n' "$(basename "$bundle" .vbvm)" \
                "$(stat -f '%Sm' -t '%Y-%m-%d %H:%M' "$bundle")"
        done
        ;;
    save)
        require_virtualbuddy_quit
        destination=$(snapshot_path "${2:-}")
        mkdir -p "$snapshots"
        clone_bundle "$VM_BUNDLE" "$destination"
        echo "saved $VM_BUNDLE as $(basename "$destination" .vbvm)"
        ;;
    reset)
        require_virtualbuddy_quit
        source=$(snapshot_path "${2:-}")
        if [[ ! -d $source ]]; then
            echo "vm-snapshot: no snapshot named '${2}'; see vm-snapshot.sh list" >&2
            exit 1
        fi
        clone_bundle "$source" "$VM_BUNDLE"
        echo "reset $VM_BUNDLE to $(basename "$source" .vbvm)"
        ;;
    *)
        echo "$usage" >&2
        exit 2
        ;;
esac
