#!/usr/bin/env bash
# Records the security state the research profile depends on, for the host or the
# guest, into a timestamped file under the evidence directory, and prints it.
#
# usage: security-state.sh host|guest LABEL [RECON]
#   RECON is the guest path of the recon executable whose `prepare-host --status`
#   to record; it defaults to the signed build in the guest checkout.
set -euo pipefail
here=$(dirname "$0")
source "$here/env.sh"

where=${1:?usage: security-state.sh host|guest LABEL [RECON]}
label=${2:?usage: security-state.sh host|guest LABEL [RECON]}
recon=${3:-\~/recon/target/signed/recon}

probe='
echo "# $(hostname -s) $(date -u +%FT%TZ)"
sw_vers
echo "--- csrutil status"; csrutil status
echo "--- csrutil authenticated-root status"; csrutil authenticated-root status 2>&1
echo "--- sysctl -n kern.bootargs"; sysctl -n kern.bootargs
echo "--- nvram boot-args"; nvram boot-args 2>&1
echo "--- nvram recon-added-boot-args"; nvram recon-added-boot-args 2>&1
'

mkdir -p "$VM_EVIDENCE/state"
out="$VM_EVIDENCE/state/$(date +%Y%m%d-%H%M%S)-$where-$label.txt"
case "$where" in
    host) bash -c "$probe" >"$out" 2>&1 ;;
    guest)
        "$here/guest-ssh.sh" "$probe
echo '--- recon ui session prepare-host --status'
test -x $recon && $recon ui session prepare-host --status 2>&1 || echo 'recon not installed at $recon'" >"$out" 2>&1
        ;;
    *)
        echo "security-state: expected host or guest, got '$where'" >&2
        exit 2
        ;;
esac
cat "$out"
