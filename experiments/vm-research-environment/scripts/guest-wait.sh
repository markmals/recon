#!/usr/bin/env bash
# Waits until the guest answers over SSH and its automatic login owns the console, so
# the GUI session that guest-gui.sh and Recon's ui commands need exists. Use it after
# vm-lifecycle.sh boot and after any guest reboot.
#
# usage: guest-wait.sh [TIMEOUT_SECONDS]   (default 180)
set -euo pipefail
here=$(dirname "$0")
source "$here/env.sh"

timeout=${1:-180}
deadline=$((SECONDS + timeout))
console_user() {
    "$here/guest-ssh.sh" -o BatchMode=yes -o ConnectTimeout=3 'stat -f %Su /dev/console' 2>/dev/null
}
until [[ $(console_user) == "$GUEST_USER" ]]; do
    if ((SECONDS >= deadline)); then
        echo "guest-wait: the guest did not log $GUEST_USER in within ${timeout}s" >&2
        exit 1
    fi
    sleep 3
done
echo "guest up: $GUEST_USER logged in at $("$here/guest-ip.sh")"
