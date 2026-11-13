#!/usr/bin/env bash
# Sends keyboard input to the guest through the VM window, finding the window by the
# VM's name each time.
#
# usage: vm-input.sh type TEXT
#        vm-input.sh key CHORD...
set -euo pipefail
here=$(dirname "$0")
source "$here/env.sh"
source "$here/vm-geometry.sh"

mode=${1:?usage: vm-input.sh type TEXT | key CHORD...}
shift
vm_activate
window=$(vm_window_json | jq .id)
case "$mode" in
    type) "$here/host-type.sh" "$window" "${1?usage: vm-input.sh type TEXT}" ;;
    key) "$here/host-key.sh" "$window" "$@" ;;
    *)
        echo "vm-input: expected type or key, got '$mode'" >&2
        exit 2
        ;;
esac
