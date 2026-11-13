# Shared settings for the VM experiment scripts. Every value can be overridden from
# the environment; nothing here is a credential.

# Host-side Recon builds. HOST_RECON observes VirtualBuddy (windows, AX tree,
# screenshots) and sends it clicks and key chords; it must hold Accessibility and
# Screen Recording on the host. The `ui click` and `ui key` commands come from the
# proposal 0007 interaction build, so point this at such a build.
: "${HOST_RECON:=$HOME/Library/Application Support/Sesh/worktrees/recon-4bd2818c/interact-with-ui-on-macos/target/signed/recon}"

: "${VM_NAME:=Recon Research}"
: "${VM_LIBRARY:=$HOME/Library/Application Support/VirtualBuddy}"
: "${VM_BUNDLE:=$VM_LIBRARY/$VM_NAME.vbvm}"

# Experiment state that must stay out of the repository: the dedicated SSH key,
# evidence captures, and downloads.
: "${VM_WORKDIR:=$HOME/ReconResearchVM}"
: "${VM_SSH_KEY:=$VM_WORKDIR/ssh/id_ed25519}"
: "${VM_EVIDENCE:=$VM_WORKDIR/evidence}"

export HOST_RECON VM_NAME VM_LIBRARY VM_BUNDLE VM_WORKDIR VM_SSH_KEY VM_EVIDENCE
