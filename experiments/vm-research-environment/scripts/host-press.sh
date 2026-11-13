#!/usr/bin/env bash
# Clicks the centre of the first accessibility element in a VirtualBuddy window whose
# description, title, or value equals LABEL, optionally only among elements of ROLE.
# VirtualBuddy's SwiftUI buttons expose their label only as AXDescription, which
# `ui click --title` cannot match, so this resolves the element's frame and sends a
# window-addressed mouse click.
#
# usage: host-press.sh WINDOW_ID LABEL [ROLE]
set -euo pipefail
source "$(dirname "$0")/env.sh"
source "$(dirname "$0")/input.sh"

window=${1:?usage: host-press.sh WINDOW_ID LABEL [ROLE]}
label=${2:?usage: host-press.sh WINDOW_ID LABEL [ROLE]}
role=${3:-}

tree=$("$HOST_RECON" ui ax tree VirtualBuddy --window "$window" --depth 40)
point=$(jq -r --arg label "$label" --arg role "$role" '
    .root as $root
    | [$root | .. | objects | select(.frame? != null)
        | select($role == "" or .role == $role)
        | select(.description == $label or .title == $label or .value == $label)][0] as $hit
    | if $hit == null then empty else
        "\($hit.frame.x + $hit.frame.width / 2 - $root.frame.x) \($hit.frame.y + $hit.frame.height / 2 - $root.frame.y)"
      end' <<<"$tree")

if [[ -z $point ]]; then
    echo "host-press: no element labelled '$label' in window $window" >&2
    exit 1
fi
read -r x y <<<"$point"
send_input click VirtualBuddy --window "$window" --x "$x" --y "$y" --method mouse
echo "pressed '$label' at ($x, $y) in window $window"
