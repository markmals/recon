#!/usr/bin/env bash
# Lists VirtualBuddy's windows, or captures one to a new PNG under the evidence
# directory and prints its path: host-screen.sh [WINDOW_ID NAME]
set -euo pipefail
source "$(dirname "$0")/env.sh"

if [[ $# -eq 0 ]]; then
    "$HOST_RECON" ui windows VirtualBuddy | jq -c '.windows[] | {id, title, bounds, onScreen}'
    exit
fi

window=$1
name=${2:?usage: host-screen.sh WINDOW_ID NAME}
mkdir -p "$VM_EVIDENCE/screens"
out="$VM_EVIDENCE/screens/$(date +%Y%m%d-%H%M%S)-$name.png"
"$HOST_RECON" ui screenshot VirtualBuddy --window "$window" --out "$out" >/dev/null
echo "$out"
