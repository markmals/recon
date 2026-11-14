#!/usr/bin/env bash
# Takes, releases, and reports the lock that keeps two agents from driving the one VM
# at once. Holding it means: you may boot, stop, reset, and inspect; nobody else may
# boot, stop, save, or reset (vm-lifecycle.sh and vm-snapshot.sh refuse them unless
# VM_OWNER names the holder). The lock is advisory for everything else.
#
# usage: vm-lock.sh status
#        vm-lock.sh acquire OWNER PURPOSE   (fails while someone else holds it)
#        vm-lock.sh release OWNER           (only the holder releases it)
#        vm-lock.sh break                   (the user's decision only: drops any holder)
set -euo pipefail
source "$(dirname "$0")/env.sh"

usage="usage: vm-lock.sh status | acquire OWNER PURPOSE | release OWNER | break"
holder="$vm_lock/holder"

holder_name() { sed -n 's/^owner: //p' "$holder" 2>/dev/null; }

action=${1:?$usage}
case "$action" in
    status)
        if [[ -d $vm_lock ]]; then
            echo "locked"
            sed 's/^/    /' "$holder"
        else
            echo "free"
        fi
        ;;
    acquire)
        owner=${2:?$usage}
        purpose=${3:?$usage}
        mkdir -p "$VM_WORKDIR"
        # mkdir either creates the lock or fails because it exists, so two callers
        # cannot both succeed.
        if ! mkdir "$vm_lock" 2>/dev/null; then
            if [[ $(holder_name) == "$owner" ]]; then
                echo "already held by $owner"
                exit 0
            fi
            echo "vm-lock: the VM is held by '$(holder_name)'; ask the user before going further" >&2
            sed 's/^/    /' "$holder" >&2
            exit 1
        fi
        printf 'owner: %s\npurpose: %s\nsince: %s\nfrom: %s\n' \
            "$owner" "$purpose" "$(date '+%Y-%m-%d %H:%M %Z')" "$PWD" >"$holder"
        echo "acquired by $owner; pass VM_OWNER='$owner' to vm-lifecycle.sh and vm-snapshot.sh"
        ;;
    release)
        owner=${2:?$usage}
        [[ -d $vm_lock ]] || { echo "already free"; exit 0; }
        if [[ $(holder_name) != "$owner" ]]; then
            echo "vm-lock: held by '$(holder_name)', not '$owner'" >&2
            exit 1
        fi
        rm -rf "$vm_lock"
        echo "released"
        ;;
    break)
        [[ -d $vm_lock ]] && sed 's/^/    dropped /' "$holder"
        rm -rf "$vm_lock"
        echo "free"
        ;;
    *)
        echo "$usage" >&2
        exit 2
        ;;
esac
