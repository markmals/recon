# Sourced by the vm-* scripts. Finds the VM's window and maps guest display points to
# window points.
#
# VirtualBuddy draws the guest display aspect-fit below a toolbar, so a guest point
# (gx, gy) on the 1280x800-point display configured by configure-vm.sh lands at
# (x0 + gx * s, y0 + gy * s) in the window.

: "${VM_TOOLBAR_HEIGHT:=52}"
: "${VM_GUEST_WIDTH:=1280}"
: "${VM_GUEST_HEIGHT:=800}"

vm_window_json() {
    "$HOST_RECON" ui windows VirtualBuddy |
        jq -ce --arg name "$VM_NAME" '[.windows[] | select(.title == $name)][0] // error("no window titled \($name)")'
}

# Prints "WINDOW_ID SCALE X0 Y0".
vm_geometry() {
    vm_window_json | jq -r \
        --argjson bar "$VM_TOOLBAR_HEIGHT" --argjson gw "$VM_GUEST_WIDTH" --argjson gh "$VM_GUEST_HEIGHT" '
        .bounds as $b
        | ([$b.width / $gw, ($b.height - $bar) / $gh] | min) as $s
        | "\(.id) \($s) \(($b.width - $gw * $s) / 2) \($bar + (($b.height - $bar) - $gh * $s) / 2)"'
}

# The VM view forwards keyboard and mouse input to the guest only while VirtualBuddy is
# the active application; window-addressed input to an inactive VirtualBuddy is
# delivered to the window and dropped by the view. Input sent in the first moments
# after activation was also lost, hence the settle delay.
vm_activate() {
    if ! lsappinfo info -only name "$(lsappinfo front)" | grep -q '"VirtualBuddy"'; then
        open -a /Applications/VirtualBuddy.app
        sleep "${VM_ACTIVATE_SETTLE:-1}"
    fi
}
