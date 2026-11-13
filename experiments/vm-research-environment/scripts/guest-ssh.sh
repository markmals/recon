#!/usr/bin/env bash
# Runs a command in the guest over SSH with the experiment's dedicated key only: no
# agent, no forwarding, no other identities, and a known_hosts file of its own.
#
# usage: guest-ssh.sh [ssh options...] [--] COMMAND...
set -euo pipefail
source "$(dirname "$0")/env.sh"

: "${GUEST_USER:=recon}"
: "${GUEST_HOST:=$("$(dirname "$0")/guest-ip.sh")}"

exec ssh \
    -F /dev/null \
    -i "$VM_SSH_KEY" \
    -o IdentitiesOnly=yes \
    -o IdentityAgent=none \
    -o ForwardAgent=no \
    -o ForwardX11=no \
    -o ClearAllForwardings=yes \
    -o UserKnownHostsFile="$VM_WORKDIR/ssh/known_hosts" \
    -o StrictHostKeyChecking=accept-new \
    -o ServerAliveInterval=15 \
    -o ConnectTimeout=10 \
    "$GUEST_USER@$GUEST_HOST" "$@"
