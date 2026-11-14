#!/usr/bin/env bash
# Prints a one-line digest per command from an inspect-target.sh evidence directory, so
# a capability is reported from what its own JSON says rather than from its exit code.
#
# usage: summarize-inspection.sh EVIDENCE_DIR
set -uo pipefail
d=${1:?usage: summarize-inspection.sh EVIDENCE_DIR}

digest() {
    local name=$1 filter=$2
    local status
    status=$(cat "$d/$name.exit" 2>/dev/null || echo missing)
    if [[ $status == 0 ]]; then
        printf '%-17s PASS  %s\n' "$name" "$(jq -c "$filter" "$d/$name.json" 2>&1 | cut -c1-220)"
    else
        printf '%-17s FAIL  exit %s %s\n' "$name" "$status" "$(jq -c '.error | {code, message}' "$d/$name.err" 2>/dev/null | cut -c1-220)"
    fi
}

digest app-inspect '{bundleId: (.bundleId // .application.bundleId), architectures: (.architectures // .executable.architectures)}'
digest preflight '[.modes[] | select(.profile == "research") | "\(.mode)=\(.status)"]'
digest open '{mode: .establishedMode, profile, reused, helperBuild: .helperBuild[0:12], arch: .identity.architecture, pid, osBuild: .identity.osBuild}'
digest windows '{windows: (.windows | length), nativeWindows: (.nativeWindows | length), nativeClass: .nativeWindows[0].basic.class}'
digest ax-tree '{root: .root.role, truncated, nodes: ([.. | objects | select(has("role"))] | length)}'
digest screenshot '{width: .image.width, height: .image.height, uniform: .image.uniform, method}'
digest view-tree '{returned: .coverage.returned, visited: .coverage.visited, complete: .coverage.complete, first: .nodes[0].basic.class[0:60]}'
digest view-tree-layers '{returned: .coverage.returned, complete: .coverage.complete, first: .nodes[0].basic.class}'
digest view-node '{kind: .node.kind, class: .node.basic.class[0:60], keys: (.node | keys)}'
digest view-find '{matches: (.nodes | length), complete: .coverage.complete}'
digest view-fonts '{fonts: [(.fonts // .inventory // [])[] | (.descriptor.postScriptName // .postScriptName // .name // .family)] | unique | .[0:6], complete: .coverage.complete}'
digest correlate '{candidates: (.candidates | length), nativeComplete: .native.coverage.complete, unmatchedAccessibility: (.unmatchedAccessibility | length)}'
digest close '{state, acknowledged, helperResident, processExited}'
[[ -f $d/view-find-identifier.exit ]] &&
    digest view-find-identifier '{identifier: "'"$(cat "$d/identifier-selection" 2>/dev/null)"'", matches: (.nodes | length), complete: .coverage.complete}'
true
