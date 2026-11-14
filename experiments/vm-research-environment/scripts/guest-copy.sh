#!/usr/bin/env bash
# Copies files between the host and the running guest with rsync over the dedicated
# SSH key: an app or a build into the guest, screenshots and other results out. A
# guest path without a leading / is relative to the guest account's home. rsync's
# trailing-slash rule applies: `push build/My.app Applications/` makes
# ~/Applications/My.app in the guest.
#
# usage: guest-copy.sh push LOCAL GUEST_PATH
#        guest-copy.sh pull GUEST_PATH LOCAL
set -euo pipefail
here=$(dirname "$0")
source "$here/env.sh"

usage="usage: guest-copy.sh push LOCAL GUEST_PATH | pull GUEST_PATH LOCAL"
: "${GUEST_HOST:=$("$here/guest-ip.sh")}"
guest="$GUEST_USER@$GUEST_HOST"

direction=${1:?$usage}
from=${2:?$usage}
to=${3:?$usage}
# openrsync hands the guest path to the guest's shell, which would split it at spaces.
case "$direction" in
    push) exec rsync -a -e "$guest_rsync_shell" "$from" "$guest:$(printf '%q' "$to")" ;;
    pull) exec rsync -a -e "$guest_rsync_shell" "$guest:$(printf '%q' "$from")" "$to" ;;
    *)
        echo "$usage" >&2
        exit 2
        ;;
esac
