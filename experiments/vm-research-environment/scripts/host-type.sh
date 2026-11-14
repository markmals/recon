#!/usr/bin/env bash
# Types text into one VirtualBuddy VM window from the host, one physical key chord per
# character, through `recon ui key`. The VM view forwards physical key codes to the
# guest, so each character becomes the US-layout key that produces it; a character
# with no such key is refused before anything is sent.
#
# usage: host-type.sh WINDOW_ID TEXT     (append `return` with host-key.sh)
set -euo pipefail
source "$(dirname "$0")/env.sh"
source "$(dirname "$0")/input.sh"

window=${1:?usage: host-type.sh WINDOW_ID TEXT}
text=${2?usage: host-type.sh WINDOW_ID TEXT}

chord_for() {
    local c=$1
    case "$c" in
        [a-z0-9]) printf '%s' "$c" ;;
        [A-Z]) printf 'shift+%s' "$(printf '%s' "$c" | tr 'A-Z' 'a-z')" ;;
        ' ') printf 'space' ;;
        '`' | '-' | '=' | '[' | ']' | '\' | ';' | "'" | ',' | '.' | '/') printf '%s' "$c" ;;
        '~') printf 'shift+`' ;;
        '!') printf 'shift+1' ;;
        '@') printf 'shift+2' ;;
        '#') printf 'shift+3' ;;
        '$') printf 'shift+4' ;;
        '%') printf 'shift+5' ;;
        '^') printf 'shift+6' ;;
        '&') printf 'shift+7' ;;
        '*') printf 'shift+8' ;;
        '(') printf 'shift+9' ;;
        ')') printf 'shift+0' ;;
        '_') printf 'shift+-' ;;
        '+') printf 'shift+=' ;;
        '{') printf 'shift+[' ;;
        '}') printf 'shift+]' ;;
        '|') printf '%s' 'shift+\' ;;
        ':') printf 'shift+;' ;;
        '"') printf "shift+'" ;;
        '<') printf 'shift+,' ;;
        '>') printf 'shift+.' ;;
        '?') printf 'shift+/' ;;
        *) return 1 ;;
    esac
}

chords=()
for ((i = 0; i < ${#text}; i++)); do
    c=${text:i:1}
    chord=$(chord_for "$c") || {
        echo "host-type: no US-layout key types '$c'" >&2
        exit 2
    }
    chords+=("$chord")
done

for chord in "${chords[@]}"; do
    send_input key VirtualBuddy --window "$window" "$chord"
done
