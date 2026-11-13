#!/usr/bin/env bash
# Prepares a freshly provisioned guest for Recon work, from the host, over SSH.
# Each step is idempotent, so the script can be rerun after a failure.
#
#   1. installs the dedicated public key (password login, once);
#   2. gives the test account passwordless sudo, keeps the guest awake, and logs it in
#      automatically so a reboot returns to a GUI session;
#   3. copies the host's Xcode 27.1 (the pinned Swift compiler) into the guest;
#   4. installs mise and clones Recon at a fixed revision from a git bundle.
#
# usage: provision-guest.sh BUNDLE REVISION
set -euo pipefail
here=$(dirname "$0")
source "$here/env.sh"

bundle=${1:?usage: provision-guest.sh BUNDLE REVISION}
revision=${2:?usage: provision-guest.sh BUNDLE REVISION}
password_file="$VM_WORKDIR/ssh/guest-password"
: "${GUEST_USER:=recon}"
: "${GUEST_HOST:=$("$here/guest-ip.sh")}"
export GUEST_USER GUEST_HOST
: "${HOST_XCODE:=/Applications/Xcode.app}"

# 1. Key-based login. SSH_ASKPASS answers the one password prompt from the file, so
# the password never appears in a process argument list.
if ! "$here/guest-ssh.sh" -o BatchMode=yes true 2>/dev/null; then
    askpass=$(mktemp)
    trap 'rm -f "$askpass"' EXIT
    printf '#!/bin/sh\ncat %q\n' "$password_file" >"$askpass"
    chmod 700 "$askpass"
    SSH_ASKPASS="$askpass" SSH_ASKPASS_REQUIRE=force \
        ssh -F /dev/null -o IdentityAgent=none -o PubkeyAuthentication=no \
        -o UserKnownHostsFile="$VM_WORKDIR/ssh/known_hosts" -o StrictHostKeyChecking=accept-new \
        "$GUEST_USER@$GUEST_HOST" \
        'umask 077; mkdir -p ~/.ssh; cat >> ~/.ssh/authorized_keys' <"$VM_SSH_KEY.pub"
fi

# 2. Passwordless sudo for the disposable test account, and no sleep: the GUI tests
# need a live, unlocked desktop.
"$here/guest-ssh.sh" "sudo -S -p '' sh -c '
    echo \"$GUEST_USER ALL=(ALL) NOPASSWD: ALL\" > /etc/sudoers.d/recon-research
    chmod 440 /etc/sudoers.d/recon-research
    visudo -cf /etc/sudoers.d/recon-research
'" <"$password_file"
"$here/guest-ssh.sh" 'sudo pmset -a sleep 0 displaysleep 0 disksleep 0 &&
    defaults -currentHost write com.apple.screensaver idleTime -int 0'

# `sysadminctl -autologin set` sets autoLoginUser but fails to write /etc/kcpassword on
# macOS 27.2.1 (SACSetAutoLoginPassword error:22), so the file is written directly in
# loginwindow's XOR format. The password arrives on standard input.
"$here/guest-ssh.sh" "sudo defaults write /Library/Preferences/com.apple.loginwindow autoLoginUser $GUEST_USER
    python3 -c '
import sys
password = sys.stdin.read().strip().encode()
key = [125, 137, 82, 35, 210, 188, 221, 234, 163, 185, 31]
if len(password) % 12:
    password += bytes(12 - len(password) % 12)
sys.stdout.buffer.write(bytes(b ^ key[i % len(key)] for i, b in enumerate(password)))
' | sudo tee /etc/kcpassword >/dev/null
    sudo chown root:wheel /etc/kcpassword && sudo chmod 600 /etc/kcpassword" <"$password_file"

# 3. Xcode 27.1 carries swiftlang-6.4.0.34.1, the compiler swift-toolchain.toml pins.
ssh_command="ssh -F /dev/null -i $VM_SSH_KEY -o IdentitiesOnly=yes -o IdentityAgent=none"
ssh_command+=" -o UserKnownHostsFile=$VM_WORKDIR/ssh/known_hosts"
rsync -a --delete -e "$ssh_command" --rsync-path='sudo rsync' \
    "$HOST_XCODE/" "$GUEST_USER@$GUEST_HOST:/Applications/Xcode.app/"
"$here/guest-ssh.sh" 'sudo xcode-select -s /Applications/Xcode.app &&
    sudo xcodebuild -license accept && sudo xcodebuild -runFirstLaunch && xcrun swiftc --version'

# 4. mise and the repository. ~/.zshenv is the one file every zsh reads, including the
# non-interactive shell that runs an SSH command.
"$here/guest-ssh.sh" 'test -x ~/.local/bin/mise || curl -fsSL https://mise.run | sh
    grep -q "mise activate" ~/.zshenv 2>/dev/null ||
        echo "eval \"\$(~/.local/bin/mise activate zsh --shims)\"" >> ~/.zshenv'
scp -F /dev/null -i "$VM_SSH_KEY" -o IdentitiesOnly=yes -o IdentityAgent=none \
    -o UserKnownHostsFile="$VM_WORKDIR/ssh/known_hosts" \
    "$bundle" "$GUEST_USER@$GUEST_HOST:recon.bundle"
"$here/guest-ssh.sh" "test -d ~/recon || git clone -q -b main ~/recon.bundle ~/recon
    cd ~/recon && git fetch -q ~/recon.bundle main && git checkout -q --detach $revision && git rev-parse HEAD"
