#!/usr/bin/env bash
# Captures the guest display from the host, cropped out of the VM window and scaled to
# guest points, so a pixel position in the image is the guest point vm-click.sh takes.
#
# usage: vm-screen.sh NAME     (prints the PNG path)
set -euo pipefail
here=$(dirname "$0")
source "$here/env.sh"
source "$here/vm-geometry.sh"

name=${1:?usage: vm-screen.sh NAME}
read -r window scale x0 y0 < <(vm_geometry)
raw=$("$here/host-screen.sh" "$window" "$name-window")
out="${raw%-window.png}.png"

# The capture is at the window's backing resolution; derive its factor from the width.
backing=$(jq -n --argjson px "$(sips -g pixelWidth "$raw" | awk '/pixelWidth/ {print $2}')" \
    --argjson pt "$(vm_window_json | jq .bounds.width)" '$px / $pt')
read -r cx cy cw ch < <(jq -rn --argjson k "$backing" --argjson s "$scale" --argjson x "$x0" \
    --argjson y "$y0" --argjson gw "$VM_GUEST_WIDTH" --argjson gh "$VM_GUEST_HEIGHT" \
    '"\($x * $k | round) \($y * $k | round) \($gw * $s * $k | round) \($gh * $s * $k | round)"')
sips --cropOffset "$cy" "$cx" -c "$ch" "$cw" "$raw" --out "$out" >/dev/null
sips -z "$VM_GUEST_HEIGHT" "$VM_GUEST_WIDTH" "$out" >/dev/null
rm -f "$raw"
echo "$out"
