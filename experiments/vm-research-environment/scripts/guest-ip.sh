#!/usr/bin/env bash
# Prints the guest's NAT address. VirtualBuddy reports no guest address, so this reads
# the MAC address from the VM's configuration and finds its lease in the host's vmnet
# DHCP server database. bootpd drops leading zeros from each octet.
set -euo pipefail
source "$(dirname "$0")/env.sh"

config="$VM_BUNDLE/.vbdata/Config.plist"
mac=$(plutil -extract hardware.networkDevices.0.macAddress raw -o - "$config")
lease_mac=$(awk -F: '{ for (i = 1; i <= NF; i++) { sub(/^0/, "", $i); printf "%s%s", (i > 1 ? ":" : ""), tolower($i) } }' <<<"$mac")

awk -v want="1,$lease_mac" '
    /^\{/ { ip = ""; hw = "" }
    /ip_address=/ { sub(/.*ip_address=/, ""); ip = $0 }
    /hw_address=/ { sub(/.*hw_address=/, ""); hw = $0 }
    /^\}/ { if (hw == want) last = ip }
    END { if (last == "") exit 1; print last }
' /var/db/dhcpd_leases
