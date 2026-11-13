#!/usr/bin/env bash
# Runs Recon's observation and native-inspection commands against one running guest
# application and keeps every command's stdout, stderr, and exit status as evidence.
#
# usage: inspect-target.sh LABEL APP_PATH MODE PROFILE
#   e.g. inspect-target.sh calculator /System/Applications/Calculator.app attach research
#
# MODE launch expects the application not to be running; existing and attach expect it
# running. The session is closed at the end. Window IDs, handles, and classes come from
# this run's own results, never from an earlier one.
set -uo pipefail
here=$(dirname "$0")
source "$here/env.sh"

label=${1:?usage: inspect-target.sh LABEL APP_PATH MODE PROFILE}
app=${2:?usage: inspect-target.sh LABEL APP_PATH MODE PROFILE}
mode=${3:?usage: inspect-target.sh LABEL APP_PATH MODE PROFILE}
profile=${4:?usage: inspect-target.sh LABEL APP_PATH MODE PROFILE}

out="$VM_EVIDENCE/matrix/$(date +%Y%m%d-%H%M%S)-$label"
mkdir -p "$out"

# One guest-side script, so the whole sequence runs in the guest's GUI session and the
# session identifier never leaves it. After a launch the target is still ordering its
# windows in, and observing during that reports the target's own window appearing as
# interference, so a launch waits before the first observation.
"$here/guest-gui.sh" "$(
    cat <<EOF
R=~/recon/target/signed/recon
A='$app'
O=/tmp/recon-inspect-$label
rm -rf "\$O"; mkdir -p "\$O"
run() { name=\$1; shift; "\$R" "\$@" >"\$O/\$name.json" 2>"\$O/\$name.err"; echo \$? >"\$O/\$name.exit"; }
run app-inspect app inspect "\$A"
run preflight ui session preflight "\$A"
run open ui session open "\$A" --mode $mode --profile $profile
[ $mode = launch ] && sleep 3
S=\$(jq -r '.session.id // .sessionId // .session // empty' "\$O/open.json" 2>/dev/null)
echo "\$S" >"\$O/session-id"
session=(); [ -n "\$S" ] && session=(--session "\$S")
run windows ui windows "\$A" "\${session[@]}"
W=\$(jq -r '[.windows[] | select(.onScreen and .layer == 0)][0].id // empty' "\$O/windows.json")
run ax-tree ui ax tree "\$A" --window "\$W" --depth 6
run screenshot ui screenshot "\$A" --window "\$W" --out "\$O/screenshot.png" "\${session[@]}"
if [ -n "\$S" ]; then
    run view-tree ui view tree "\$A" --session "\$S" --window "\$W" --depth 4 --limit 400
    run view-tree-layers ui view tree "\$A" --session "\$S" --window "\$W" --kind layer --depth 3 --limit 200
    # The view with the shortest runtime class name is the most legible find target.
    jq -r '[.nodes[] | select(.kind == "view" and (.basic.class | type) == "string")]
        | sort_by(.basic.class | length) | .[0] | "\(.handle) \(.basic.class)"' \
        "\$O/view-tree.json" >"\$O/node-selection"
    read -r H C <"\$O/node-selection"
    run view-node ui view node "\$A" "\$H" --session "\$S" --fields basic,layout,font,layer,cell
    run view-find ui view find "\$A" --session "\$S" --window "\$W" --class "\$C" --depth 64 --limit 50
    I=\$(jq -r '[.. | objects | .identifier? | objects | .value? // empty | strings | select(length > 0)][0] // empty' "\$O/view-tree.json")
    echo "\$I" >"\$O/identifier-selection"
    [ -n "\$I" ] && run view-find-identifier ui view find "\$A" --session "\$S" --window "\$W" --identifier "\$I" --depth 64 --limit 50
    run view-fonts ui view fonts "\$A" --session "\$S" --window "\$W" --depth 64
    run correlate ui correlate "\$A" --session "\$S" --window "\$W" --depth 8 --limit 100
    run close ui session close "\$S"
fi
for f in "\$O"/*.exit; do printf '%-18s exit %s\n' "\$(basename "\$f" .exit)" "\$(cat "\$f")"; done
EOF
)"
"$here/guest-ssh.sh" "tar -C /tmp -cf - recon-inspect-$label" | tar -C "$out" -xf -
echo "evidence: $out/recon-inspect-$label"
