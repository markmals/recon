# Sourced by the host-* input scripts.
#
# send_input ARGS... runs `$HOST_RECON ui ARGS...`. Recon's interaction guard reports
# UI_INTERFERENCE when anything moves during the operation, including VirtualBuddy
# itself opening the window a click asked for. When the input was fully released and
# the only disturbance is the target application's own window, the input was
# delivered and the reaction is the expected effect, so that case is reported and
# accepted. Every other failure stops the caller.
send_input() {
    local stderr status
    stderr=$(mktemp)
    "$HOST_RECON" ui "$@" >/dev/null 2>"$stderr"
    status=$?
    if [[ $status -ne 0 ]] && jq -e '
        .error.code == "UI_INTERFERENCE"
        and .error.details.release == "complete"
        and (.error.details.disturbance | test("which is the instance this request selected"))
    ' "$stderr" >/dev/null 2>&1; then
        echo "delivered; target reacted: $(jq -r .error.details.disturbance "$stderr")" >&2
        status=0
    elif [[ $status -ne 0 ]]; then
        cat "$stderr" >&2
    fi
    rm -f "$stderr"
    return "$status"
}
