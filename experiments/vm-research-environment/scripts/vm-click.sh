#!/usr/bin/env bash
# Clicks a guest display point (as read from vm-screen.sh) through the VM window.
#
# usage: vm-click.sh GX GY [extra `ui click` options, e.g. --count 2]
set -euo pipefail
here=$(dirname "$0")
source "$here/env.sh"
source "$here/input.sh"
source "$here/vm-geometry.sh"

gx=${1:?usage: vm-click.sh GX GY [options]}
gy=${2:?usage: vm-click.sh GX GY [options]}
shift 2
vm_activate
read -r window scale x0 y0 < <(vm_geometry)
read -r x y < <(jq -rn --argjson s "$scale" --argjson x0 "$x0" --argjson y0 "$y0" \
    --argjson gx "$gx" --argjson gy "$gy" '"\($x0 + $gx * $s) \($y0 + $gy * $s)"')
send_input click VirtualBuddy --window "$window" --x "$x" --y "$y" --method mouse "$@"
