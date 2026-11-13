#!/usr/bin/env bash
# Authorizes research attaches for the current guest boot, before any real target.
#
# The first attach to a non-debuggable process in each boot makes taskgated evaluate
# system.privilege.taskport, and the guest shows "Developer Tools Access is trying to
# take control of another process". Recon's open gives up after ten seconds and refuses
# that process again until it exits (recon-y9v). So this spends the prompt on a
# throwaway Chess process: it starts an attach, waits for the prompt, answers it with
# the guest password through the VM window, and quits Chess. authd then caches the
# shared right for the rest of the boot.
#
# usage: authorize-taskport.sh
set -euo pipefail
here=$(dirname "$0")
source "$here/env.sh"

# authd's log is kept across boots, so only entries since this boot count.
authorized() {
    "$here/guest-ssh.sh" 'since=$(date -r "$(sysctl -n kern.boottime | awk -F "[ ,]+" "{print \$4}")" "+%Y-%m-%d %H:%M:%S")
        sudo log show --start "$since" --predicate "process == \"authd\"" --style compact 2>/dev/null |
            grep -q "Succeeded authorizing right .system.privilege.taskport."'
}

if authorized; then
    echo "system.privilege.taskport is already authorized in this boot"
    exit 0
fi

"$here/guest-gui.sh" 'open -g -a Chess; sleep 3
    nohup target/signed/recon ui session open Chess --mode attach --profile research >/dev/null 2>&1 &' >/dev/null

for _ in $(seq 1 20); do
    "$here/guest-ssh.sh" 'pgrep -x SecurityAgent >/dev/null' && break
    sleep 1
done
if ! "$here/guest-ssh.sh" 'pgrep -x SecurityAgent >/dev/null'; then
    echo "authorize-taskport: no authorization prompt appeared" >&2
    exit 1
fi

# The prompt is centred on the 1280x800-point guest display; its password field
# takes focus, and a click there makes sure.
sleep 2
"$here/vm-click.sh" 640 360
"$here/vm-input.sh" type "$(cat "$VM_WORKDIR/ssh/guest-password")"
"$here/vm-input.sh" key return
sleep 2
"$here/guest-ssh.sh" 'pkill -x Chess' || true

if authorized; then
    echo "system.privilege.taskport authorized for this boot"
else
    echo "authorize-taskport: authd recorded no successful taskport authorization" >&2
    "$here/vm-screen.sh" authorize-taskport-failure >&2
    exit 1
fi
