#!/usr/bin/env bash
# Drives the VM's lifecycle through VirtualBuddy's deep links.
#
# usage: vm-lifecycle.sh boot|recovery|stop|open|library
#
# VirtualBuddy identifies the caller by the audit token of the Apple Event that carries
# the URL, so the sender must still be alive when VirtualBuddy checks it. `open -W`
# keeps /usr/bin/open alive until VirtualBuddy exits, so it runs in the background and
# is reaped after VirtualBuddy has had time to read the event. The first call from an
# unapproved client shows VirtualBuddy's own "Allow / Don't Allow" panel.
set -euo pipefail
source "$(dirname "$0")/env.sh"

action=${1:?usage: vm-lifecycle.sh boot|recovery|stop|open|library}
name=$(jq -rn --arg n "$VM_NAME" '$n | @uri')

case "$action" in
    boot) url="virtualbuddy://boot?name=$name" ;;
    recovery) url="virtualbuddy://boot?name=$name&options.bootInRecoveryMode=true" ;;
    stop) url="virtualbuddy://stop?name=$name" ;;
    open) url="virtualbuddy://open?name=$name" ;;
    library) url="virtualbuddy://library" ;;
    *)
        echo "vm-lifecycle: unknown action '$action'" >&2
        exit 2
        ;;
esac

open -g -W "$url" &
sender=$!
sleep "${VM_LINK_GRACE:-5}"
kill "$sender" 2>/dev/null || true
wait "$sender" 2>/dev/null || true
echo "sent $url"
