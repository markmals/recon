---
name: inspecting-apps-in-the-research-vm
description: Use when inspecting a macOS app with Recon's research profile (native view trees, attach or launch injection, Apple or sandboxed apps) without weakening this Mac, or when the user mentions the research VM, the Recon Research VM, the VirtualBuddy guest, or the demo VM.
---

# Inspecting apps in the research VM

## Overview

"Recon Research" is a VirtualBuddy VM on this Mac: macOS 27.2.1 with SIP off, Recon's research boot-args active, and Recon built at `~/recon/target/signed/recon` inside the guest. Invasive inspection runs there, never on this Mac. Scripts drive it from the host.

- Scripts: `~/Developer/CLIs/recon/experiments/vm-research-environment/scripts` (run them from that directory)
- State: `~/Library/Application Support/ReconResearchVM` (layout in its `README.md`)
- Snapshots: `research-ready` (clean), `demo` (a fictional person's data in Messages, Photos, Calendar, Contacts, Notes, Reminders), `installed-unbooted`

A second guest runs macOS 26.6.2 with the same account, SIP off and research boot-args. Put `VM_NAME="Recon Research 26"` before any script; its snapshots are `tahoe26-installed-unbooted`, `tahoe26-provisioned` (SIP on) and `tahoe26-research-ready`. It carries no current Recon build: push one with `guest-copy.sh`, grant it, and run `prepare-host` (the experiment `README.md` section "A macOS 26 guest" says how). The one lock covers both guests, so check it before booting either.

## Workflow

```sh
cd ~/Developer/CLIs/recon/experiments/vm-research-environment/scripts
./vm-lock.sh status                                # locked by someone else: stop, ask the user
pgrep -x VirtualBuddy                              # running means the VM may be up (there is no status action)
./vm-lock.sh acquire my-project "inspect MyApp"    # owner = your project or session name
VM_OWNER=my-project ./vm-snapshot.sh reset research-ready   # or demo
VM_OWNER=my-project ./vm-lifecycle.sh boot && ./guest-wait.sh
./guest-gui.sh 'open -g -a Calculator'            # starts an app in the guest's GUI session
./inspect-target.sh calc /System/Applications/Calculator.app attach research
./summarize-inspection.sh "<evidence dir it printed>"
./guest-gui.sh '~/recon/target/signed/recon ui windows com.apple.calculator'   # any Recon command
./guest-ssh.sh 'sudo shutdown -h now'; sleep 15; osascript -e 'quit app "VirtualBuddy"'
./vm-lock.sh release my-project
```

`vm-lifecycle.sh` and `vm-snapshot.sh` refuse while the lock is held by someone other than `VM_OWNER`. `inspect-target.sh` MODE is `launch` (app not running) or `attach`/`existing` (running). It is a fixed smoke run of every inspection command, with the view tree cut at `--depth 4 --limit 400`, and it writes each result under `evidence/matrix/` in the state directory. For a complete tree, open a session and run `ui view tree APP --session S --window W --depth 64 --limit 5000` yourself.

Run Recon's `ui` commands through `guest-gui.sh`: they need the guest's logged-in GUI session, which a plain `guest-ssh.sh` command does not reach. Both scripts take the guest command as one string, run by zsh in the guest, and pass its output back.

Your own app: build it on this Mac, then `./guest-copy.sh push path/My.app Applications/` and inspect `/Users/recon/Applications/My.app`. `./guest-copy.sh pull GUEST_PATH LOCAL` brings files back; guest paths are relative to the guest home.

## Rules

- Before `vm-snapshot.sh save`, get the user's explicit OK: it overwrites a shared snapshot.
- A free lock means the VM's current disk is disposable: once you hold the lock, reset it without asking. Releasing the lock gives up whatever you left in the VM, so if it matters, ask the user to save it as a snapshot first.
- Never run `prepare-host`, `csrutil`, `nvram`, or Recon's research profile on this Mac.
- When done: VM shut down, VirtualBuddy quit, lock released.

## Limits

| Symptom | Cause |
| --- | --- |
| `ui session open` on Messages times out | Messages' sandbox blocks Recon's helper (`recon-efk`) |
| No privacy prompt for a non-Apple app in the guest; access denied | Prompts are suppressed while the research boot-args are active (`recon-pr1`); see `demo/README.md` |
| A dialog, Recovery, or System Settings needs a click | `./vm-screen.sh NAME` (PNG in guest points), `./vm-click.sh X Y`, `./vm-input.sh`. These bring VirtualBuddy to the front, carry no modifier keys, and need the host grants below |
| `vm-screen.sh` or `vm-click.sh` fails with `PERMISSION_DENIED` | `bin/recon` in the state directory needs Screen Recording, Accessibility, and Device Control on this Mac; ask the user |
| Need a newer Recon in the guest | "Reusing the configured guest" in the experiment `README.md` |

The guest can reach this Mac's and the LAN's network services. Background: `experiments/vm-research-environment/README.md` and `demo/README.md`.
