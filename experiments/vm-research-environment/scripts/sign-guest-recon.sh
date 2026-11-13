#!/usr/bin/env bash
# Does what `mise run recon:sign` does for the guest's build, without putting a signing
# key in the guest: copies the guest's target/release/recon to the host, signs it there
# with RECON_SIGNING_IDENTITY and Recon's identifier, refuses an ad-hoc or team-less
# result as tools/sign-recon.ts does, and renames it into the guest's
# target/signed/recon.
#
# usage: RECON_SIGNING_IDENTITY='Developer ID Application: …' sign-guest-recon.sh
set -euo pipefail
here=$(dirname "$0")
source "$here/env.sh"

identity=${RECON_SIGNING_IDENTITY:?set RECON_SIGNING_IDENTITY to a Developer ID Application identity}
if [[ $identity == "-" ]]; then
    echo "sign-guest-recon: '-' asks for an ad-hoc signature, which loses privacy grants" >&2
    exit 1
fi

: "${GUEST_HOST:=$("$here/guest-ip.sh")}"
remote="recon@$GUEST_HOST"
scp_opts=(-q -F /dev/null -i "$VM_SSH_KEY" -o IdentitiesOnly=yes -o IdentityAgent=none
    -o UserKnownHostsFile="$VM_WORKDIR/ssh/known_hosts")

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
staged="$work/recon"

scp "${scp_opts[@]}" "$remote:recon/target/release/recon" "$staged"
codesign --force --sign "$identity" --identifier me.malstrom.recon "$staged"
description=$(codesign -dv "$staged" 2>&1)
if grep -q 'flags=[^ ]*adhoc' <<<"$description" || ! grep -q '^TeamIdentifier=[A-Z0-9]' <<<"$description"; then
    echo "sign-guest-recon: the signature is ad hoc or names no team" >&2
    exit 1
fi

"$here/guest-ssh.sh" 'mkdir -p ~/recon/target/signed'
scp "${scp_opts[@]}" "$staged" "$remote:recon/target/signed/recon.staged"
"$here/guest-ssh.sh" 'cd ~/recon/target/signed && mv recon.staged recon && codesign -dr - recon'
