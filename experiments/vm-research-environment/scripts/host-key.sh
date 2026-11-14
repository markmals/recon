#!/usr/bin/env bash
# Sends key chords, in order, to one VirtualBuddy window: host-key.sh 338 cmd+q return
set -euo pipefail
source "$(dirname "$0")/env.sh"
source "$(dirname "$0")/input.sh"

window=${1:?usage: host-key.sh WINDOW_ID CHORD...}
shift
for chord in "$@"; do
    send_input key VirtualBuddy --window "$window" "$chord"
done
