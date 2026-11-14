# Shared settings for the VM scripts. Every value can be overridden from the
# environment; nothing here is a credential.

: "${VM_NAME:=Recon Research}"
: "${VM_LIBRARY:=$HOME/Library/Application Support/VirtualBuddy}"
: "${VM_BUNDLE:=$VM_LIBRARY/$VM_NAME.vbvm}"

# Machine-local state that stays out of the repository: the dedicated SSH key and
# guest password, snapshots, the restore image, demo media, the host-control Recon,
# evidence captures, and the lock. README.md there describes the layout.
: "${VM_WORKDIR:=$HOME/Library/Application Support/ReconResearchVM}"
: "${VM_SSH_KEY:=$VM_WORKDIR/ssh/id_ed25519}"
: "${VM_EVIDENCE:=$VM_WORKDIR/evidence}"

# The host-side Recon that observes VirtualBuddy (windows, AX tree, screenshots) and
# sends it clicks and key chords. It must hold Accessibility and Screen Recording on
# the host, and those grants belong to this path. `ui click` and `ui key` come from
# the proposal 0007 interaction build, which no Homebrew release has yet.
: "${HOST_RECON:=$VM_WORKDIR/bin/recon}"

: "${GUEST_USER:=recon}"

export HOST_RECON VM_NAME VM_LIBRARY VM_BUNDLE VM_WORKDIR VM_SSH_KEY VM_EVIDENCE GUEST_USER

# Every connection to the guest uses only the dedicated key: no agent, no user SSH
# configuration, and a known_hosts file of its own. ssh reads UserKnownHostsFile as a
# whitespace-separated list, so the path is quoted inside the value, and rsync keeps
# the quotes of its -e string intact.
guest_ssh_options=(
    -F /dev/null -i "$VM_SSH_KEY" -o IdentitiesOnly=yes -o IdentityAgent=none
    -o "UserKnownHostsFile=\"$VM_WORKDIR/ssh/known_hosts\""
)
guest_rsync_shell="ssh -F /dev/null -i \"$VM_SSH_KEY\" -o IdentitiesOnly=yes -o IdentityAgent=none"
guest_rsync_shell+=" -o 'UserKnownHostsFile=\"$VM_WORKDIR/ssh/known_hosts\"'"

# The lock that keeps two agents from driving the one VM at once. vm-lock.sh takes and
# releases it; the scripts that boot, stop, save, or reset the VM call
# require_vm_owner, which refuses while someone other than $VM_OWNER holds it.
vm_lock="$VM_WORKDIR/lock"

require_vm_owner() {
    [[ -d $vm_lock ]] || return 0
    local owner
    owner=$(sed -n 's/^owner: //p' "$vm_lock/holder" 2>/dev/null)
    if [[ ${VM_OWNER:-} != "$owner" ]]; then
        echo "$(basename "$0"): the VM is locked by '$owner'. Set VM_OWNER='$owner' if that" \
            "is you; otherwise ask the user. Holder:" >&2
        sed 's/^/    /' "$vm_lock/holder" >&2
        exit 1
    fi
}
