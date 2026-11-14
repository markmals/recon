#!/usr/bin/env bash
# Runs one of Recon's mise tasks (or any command) in the guest's GUI session and keeps
# its full output and exit status under the evidence directory.
#
# usage: run-guest-suite.sh NAME 'COMMAND'
#   e.g. run-guest-suite.sh test-ui 'mise run test:ui'
set -uo pipefail
here=$(dirname "$0")
source "$here/env.sh"

name=${1:?usage: run-guest-suite.sh NAME COMMAND}
command=${2:?usage: run-guest-suite.sh NAME COMMAND}

mkdir -p "$VM_EVIDENCE/suites"
log="$VM_EVIDENCE/suites/$(date +%Y%m%d-%H%M%S)-$name.log"
{
    echo "# command: $command"
    echo "# started: $(date -u +%FT%TZ)"
    "$here/guest-ssh.sh" 'echo "# guest: $(sw_vers -productVersion) ($(sw_vers -buildVersion)); recon $(git -C ~/recon rev-parse --short HEAD)"'
} >"$log" 2>&1
"$here/guest-gui.sh" "$command" >>"$log" 2>&1
status=$?
echo "# exit: $status" >>"$log"
echo "# finished: $(date -u +%FT%TZ)" >>"$log"
tail -n 25 "$log"
echo "log: $log"
exit "$status"
