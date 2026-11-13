#!/usr/bin/env bash
# Runs a command in the guest user's logged-in GUI (Aqua) session rather than the SSH
# session. WindowServer, Accessibility, and LaunchServices lookups from a plain SSH
# session reach a different bootstrap namespace, so anything that observes or
# launches apps goes through `launchctl asuser`. Requires the guest's passwordless
# sudo from provision-guest.sh.
#
# usage: guest-gui.sh 'COMMAND'     (run by zsh -lc as the guest user, in ~/recon)
set -euo pipefail
here=$(dirname "$0")

command=${1:?usage: guest-gui.sh COMMAND}
quoted=$(printf '%q' "cd ~/recon 2>/dev/null; $command")
"$here/guest-ssh.sh" "sudo launchctl asuser \$(id -u) sudo -u \$(id -un) -H zsh -lc $quoted"
