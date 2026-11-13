# Recon in a VirtualBuddy macOS 27 guest: feasibility report

**Conclusion: viable with limitations.** Recon's complete native inspection, including the research profile, works inside a macOS 27.2.1 guest under VirtualBuddy 2.2.1. That covers attach and launch into protected Apple platform binaries (Calculator, TextEdit, Finder, all arm64e) and into a non-cooperative hardened third-party app, with and without App Sandbox (CotEditor, Hex Fiend). The guest's security was lowered entirely from inside the guest: `csrutil disable` in guest Recovery, then Recon's own `prepare-host` for the AMFI boot-args. The host's security settings did not change. The limitations are about operating the guest unattended, not about inspection, and none of them requires weakening the host:

- Guest Recovery and macOS privacy approvals still need clicks and typing in the VM window.
- Host-to-guest keyboard input carries no modifier keys.
- Each guest boot needs one interactive `system.privilege.taskport` authentication before the first research attach.

The experiment ran on 13 November 2026 against Recon `549a1e4` (`main`). The guest it built is still on this Mac at `~/Library/Application Support/VirtualBuddy/Recon Research.vbvm`. A second snapshot, `demo`, fills that guest's Apple apps with a fictional person's photos, messages, calendar, contacts, notes, and reminders for realistic inspections; [`demo/README.md`](demo/README.md) describes it.

## Environment

| Item | Value |
| --- | --- |
| Host | MacBook Pro `Mac16,1`, Apple M4 (4P + 6E), 32 GB, macOS 27.2.1 (26B131) |
| Host security | SIP disabled; `amfi_get_out_of_my_way=0x1 -arm64e_preview_abi tss_should_crash=0` active; Developer Tools access enabled; application firewall off. The host was already the research Mac, and these values were the same at the start (`evidence/state/*-host-baseline.txt`) and at the end (`state/*-host-final.txt`). |
| VirtualBuddy | 2.2.1 (430) from the GitHub release DMG, SHA-256 `16fac6dddbb012548911c0a6807a882e8df5b5b4897a218923b108aaa333fc63`; notarized, Developer ID Buddy Software LTDA (8C7439RJLG) |
| Restore image | `UniversalMac_27.2.1_26B131_Restore.ipsw`, 25,595,954,706 bytes. It is what `VZMacOSRestoreImage.fetchLatestSupported` returned, and `unzip -t` found it clean. |
| VM "Recon Research" | 5 vCPU, 16 GB, 126 GB raw disk, 2560×1600 @ 218 ppi (1280×800 points), NAT, no sound devices, no shared folders, no clipboard sharing, no VirtualBuddyGuest ([`vm-config.json`](evidence/vm-config.json)) |
| Guest | macOS 27.2.1 (26B131); one admin account `recon` with automatic login |
| Guest toolchain | Host's `/Applications/Xcode.app` (27.1, `swiftlang-6.4.0.34.1`, the pinned compiler) copied in; mise 2026.11.7 with the pins in `mise.toml` |
| Host controller | Recon itself. Homebrew `recon` 0.1.5 and the proposal 0007 interaction build (`7004ba5`, SHA-256 `d769aa48…`) already held Accessibility and Screen Recording on this host. |

## How the environment was built and driven

Every step below was performed by the agent. The scripts are in `scripts/`, and machine-local state (SSH key, guest password, downloads, baseline clone, raw evidence) stays in `~/ReconResearchVM`, outside the repository.

### Host-side control is Recon

- **Observation.** `recon ui windows|ax tree|screenshot VirtualBuddy` reads VirtualBuddy's windows, accessibility tree, and pixels without activating it (`host-screen.sh`). `vm-screen.sh` crops the guest display out of the VM window at guest-point resolution, so coordinates read from the image are guest points.
- **Input.** The interaction build's `ui click`, `ui scroll` and `ui key` send window-addressed events. They drove the whole VirtualBuddy setup wizard in the background (`host-press.sh`, `host-key.sh`, `host-type.sh`) and all guest GUI work (`vm-click.sh`, `vm-input.sh`).
- **Lifecycle.** VirtualBuddy deep links, sent with `open -g -W` (`vm-lifecycle.sh`). The first link showed VirtualBuddy's panel "“open” would like to access and control your virtual machines", and the agent clicked **Allow**. VirtualBuddy identifies the client as `/usr/bin/open`, so any local process that uses `open` can now boot, stop, and open VMs. Revoke it in VirtualBuddy Settings → Automation. `boot`, `boot … options.bootInRecoveryMode=true`, and `stop` all worked. `stop` is a graceful request, and the guest answers it with its own "shut down now?" dialog, so `sudo shutdown -h now` over SSH is the dependable stop.

### Guest creation and access

1. The IPSW was placed in VirtualBuddy's `_Downloads` folder, where the catalog row for 27.2.1 picks it up as a local file. The wizard's **Local File** route opens an `NSOpenPanel` whose content is out-of-process, and window-addressed input could not reach it.
2. VirtualBuddy's account provisioning (VZ guest provisioning: account, SSH, auto-login) could not be filled in. Its password fields enable host secure event input, and Recon's `ui key` refuses with `ACTION_UNAVAILABLE: secure event input is enabled`. Writing the same keychain item with `security add-generic-password -T VirtualBuddy` got as far as a host keychain prompt that needs the host login password. The user chose to fall back to Setup Assistant.
3. Setup Assistant, the account, Remote Login, and both privacy grants were done by clicking and typing in the VM window (`evidence/screens/`). Analytics, Location Services, and FileVault were left off, and no Apple account was signed in.
4. `provision-guest.sh` does the rest over SSH:
    - installs the dedicated key, using password login once;
    - grants the test account passwordless sudo;
    - turns off sleep and turns on automatic login (it writes `/etc/kcpassword`, because `sysadminctl -autologin` fails with `SACSetAutoLoginPassword error:22`);
    - copies Xcode 27.1 into the guest;
    - installs mise and clones Recon at `549a1e4` from a git bundle.
5. `guest-ssh.sh` uses only the dedicated key, with no agent, forwarding, or user SSH config. `guest-gui.sh` runs a command in the guest's logged-in Aqua session through `launchctl asuser`.

### Guest security posture

| Step | Command and observed result |
| --- | --- |
| Baseline | `csrutil status` enabled; `kern.bootargs` empty; the ad-hoc `recon-loader` with `com.apple.private.thread-set-state` is killed at exec with `SIGKILL (Code Signature Invalid)`, `Taskgated Invalid Signature` ([`amfi-enforced-loader.txt`](evidence/amfi-enforced-loader.txt)) |
| Recovery | Deep-link Recovery boot → Options → Continue → Utilities → Terminal. `csrutil disable` → `y` → guest password → "System Integrity Protection is off." → `reboot` ([`07-recovery-csrutil-disable.jpg`](evidence/screens/07-recovery-csrutil-disable.jpg)) |
| `prepare-host --status` | `readiness: blocked`, `sipDisabled: true`, `amfiDisabled: false` |
| `prepare-host` | `sudo nvram` accepted all three tokens and the `recon-added-boot-args` record; the helper was staged in `/Library/Application Support/Recon` |
| After reboot | `kern.bootargs` holds all three tokens; `readiness: eligible`, `stagedHelper: true`; the same ad-hoc loader now runs |
| Persistence | A cold stop plus deep-link boot kept SIP off, the boot-args, the staged helper, automatic login, SSH, and both privacy grants (`evidence/state/`) |

`csrutil authenticated-root` stayed enabled; Recon does not need it off. VirtualBuddy's host-side NVRAM editing was not used.

### Recon build, signing, and grants

The guest built the CLI, the universal arm64/arm64e helper (build `571291a7…`), the loader, and both fixtures with the repository's own tasks (`evidence/suites/*-build-native.log`). `recon:sign` needs a Developer ID identity in the keychain, and copying that private key into a weakened guest would defeat the experiment. `sign-guest-recon.sh` instead signs the guest's `target/release/recon` on the host with the same flags and checks, then puts it at the guest's `target/signed/recon`. `recon context` in the guest reports team `647GR2UNM5`, `responsibleForSelf: true`, and both permissions granted.

## Results

### Automation acceptance test

| Step                       | Mechanism                                                | Result |
| -------------------------- | -------------------------------------------------------- | ------ |
| Start the VM               | `vm-lifecycle.sh boot`                                   | PASS   |
| Observe that it booted     | `vm-screen.sh`; SSH `uptime`                             | PASS   |
| Run a command in the guest | `guest-ssh.sh`                                           | PASS   |
| Launch a GUI app           | `guest-gui.sh 'open -g -a Calculator'`                   | PASS   |
| Observe its window         | Guest `recon ui windows/screenshot`; host `vm-screen.sh` | PASS   |
| Restart the VM             | `sudo shutdown -r now`; stop plus deep-link boot         | PASS   |
| Boot into Recovery         | `vm-lifecycle.sh recovery`                               | PASS   |
| Use Recovery Terminal      | `vm-click.sh`/`vm-input.sh` through the VM window        | PASS   |
| Return to normal macOS     | `reboot` in Recovery Terminal                            | PASS   |
| Resume guest commands      | `guest-ssh.sh`                                           | PASS   |

### Recon's test suites in the guest

| Task | Result |
| --- | --- |
| `mise run check` | PASS: tools 167 passed, 2 skipped; Rust 444 passed, 27 ignored (the live tests); clippy, fmt, rustdoc, validate, and docs build clean |
| `mise run swift:test` | PASS (SDK and AppKit extension suites) |
| `mise run test:helper` | PASS, 50 tests |
| `mise run test:inject` | PASS, 5 tests (arm64 and arm64e fixture) |
| `mise run test:inspection` | PASS (`live`, `appkit`, `inspection_close_cli`) |
| `mise run test:ui` | Task BLOCKED as expected: `recon:sign` needs `RECON_SIGNING_IDENTITY`. The task's own commands against the host-signed binary: 9 passed, 1 failed. `a_screenshot_without_a_window_captures_the_applications_main_window` got the fixture's Inspector window as the AX main window, in 3 of 3 guest runs and also on the physical host, so it is not a VM effect (tracked as `recon-cks`). |
| `mise run test:focus` | The task's commands, run hands-off: PASS, 2 tests |

Logs: `evidence/suites/`.

### Capability matrix

Fixture = `ReconFixture` (cooperative, `get-task-allow`, arm64). Apple = Calculator, TextEdit, Finder (platform, arm64e, research attach). Third-party = Hex Fiend 2.18.1 (hardened, not sandboxed) and CotEditor 7.1.3 (hardened, App Sandbox), both installed unmodified. Raw JSON is in `evidence/matrix/`; `summarize-inspection.sh` reproduces the digests.

| Capability | Fixture | Apple | Third-party |
| --- | --- | --- | --- |
| Application discovery (`app list`, `app inspect`) | PASS | PASS | PASS |
| Inspection and launch (`app launch`, `--mode launch`) | PASS | PASS (TextEdit `app launch`; Calculator research launch) | PASS (Hex Fiend research launch) |
| Window enumeration | PASS | PASS | PASS |
| Accessibility tree | PASS | PASS for Calculator and TextEdit. Finder: app tree PASS, window element FAIL with `read-failed` / `axError -25200`, with or without the helper; not classified, see below | PASS |
| Screenshots | PASS | PASS | PASS |
| Session preflight | PASS | PASS (research attach `unknown`, standard `blocked`) | PASS (same) |
| Standard session | PASS (launch, existing, attach) | N/A: refused by design ("target requires the research profile") | N/A: refused by design, `INSPECTION_UNAVAILABLE` at `preflight` |
| Research session | PASS | PASS | PASS |
| Attach/launch injection | PASS | PASS (attach and launch, arm64e) | PASS (attach and launch; sandboxed CotEditor loads the staged helper) |
| Native window inspection | PASS | PASS | PASS |
| Native view/layer tree | PASS | PASS | PASS |
| Native node inspection | PASS | PASS | PASS |
| Class / identifier lookup | PASS / N/A (fixture views have no identifiers) | PASS / PASS (`_NS:36`, `_NS:13`) | PASS / PASS (`_NS:8`, `WindowContentSplitView`) |
| Font observations | PASS | PASS (TextEdit `Menlo-Regular`, Finder SF; Calculator's SwiftUI window is complete and empty) | PASS |
| Native/AX correlation | PASS | PASS (Calculator 6, TextEdit 4 candidates; Finder 0, because its window AX element failed) | PASS (4 and 13 candidates) |
| Session close/reconciliation | PASS | PASS, including the recovery path below | PASS |

The sandbox guard behaved as documented. With the staged helper moved away, attaching to CotEditor returned `INSPECTION_UNAVAILABLE` at stage `attach`, naming the packaged helper path and `prepare-host`, and nothing was loaded ([`coteditor-unstaged-helper.txt`](evidence/matrix/coteditor-unstaged-helper.txt)).

## Findings

### The taskport authorization prompt (Recon defect `recon-y9v`; macOS behaviour)

The first research attach in each guest boot made `taskgated` evaluate `system.privilege.taskport`, a `user`-class right with `authenticate-user`. The guest then showed "Developer Tools Access is trying to take control of another process" ([`10-developer-tools-access-dialog.jpg`](evidence/screens/10-developer-tools-access-dialog.jpg), [`11-developer-tools-access-after-reboot.jpg`](evidence/screens/11-developer-tools-access-after-reboot.jpg)). Recon reported `UI_INTERFERENCE` together with `InspectionTimeout` (the ten-second open deadline). Its next attach to the same process returned `INSPECTION_BUSY` ("unreachable after an unfinished attach") until that process exited, exactly as the recovery contract says. `DevToolsSecurity -enable` did not prevent the prompt. Entering the guest password once cached the shared right (36,000 s) for that boot, and every attach after that succeeded with no dialog. Preflight nevertheless reports `attachWithoutDialog: true` unconditionally (`lifecycle.rs` hardcodes it). Recon should either report this prerequisite or name it in the research-host guide. Nothing here is specific to the VM.

### Virtualization-specific observations

- **Process protection and private entitlements.** Without the boot-args, the guest enforced AMFI exactly as on hardware, killing the ad-hoc loader. With them, the kernel honoured `com.apple.private.thread-set-state` for injection into arm64e platform binaries. Guest NVRAM writes persisted in the VM's auxiliary storage across warm and cold restarts, and the kernel applied them.
- **WindowServer, Accessibility, graphics.** Window lists, SkyLight hardware capture (non-uniform images at backing scale 2), AX trees, and the no-focus evidence (`test:focus`) all behaved as on hardware. AppKit classes and fonts matched real macOS: SwiftUI hosting views, `NSKVONotifying_NSWindow`, `.SFNS-Regular`, `Menlo-Regular`.
- **Host input reaches the guest only while VirtualBuddy is the active app** (VirtualBuddy / Virtualization.framework). Window-addressed events sent to a background VirtualBuddy arrive at the window, and the VM view drops them. `vm_activate` brings VirtualBuddy forward first, which takes the host's foreground. Recon's "never activate" policy for observation is unaffected: in-guest Recon runs entirely inside the guest.
- **Modifier keys never reach the guest** (VirtualBuddy / Virtualization.framework, possibly Recon's event shape). `shift+r` typed `r`, and `cmd+space` did nothing. All guest typing was therefore limited to unshifted characters: a lowercase-only guest password, `/` to open Go to Folder, and lowercase paths, which work because APFS is case-insensitive. Anything that needs shifted characters goes over SSH instead.
- **Lost first click.** The first click after activation, or after a large pointer jump, was sometimes lost. Every step was verified from a screenshot before the next.
- **Guest-local GUI automation over SSH is weak.** Processes started under `launchctl asuser` from SSH are attributed to `sshd-keygen-wrapper` / `com.apple.sshd-session` for privacy decisions. `osascript` and `screencapture` produced Automation and screen-capture prompts there, all declined. Recon is unaffected because it disclaims responsibility and uses its own grants.
- **Not classified: Finder's window AX element** (`axError -25200`). It failed whether or not the helper was resident. There was no host Finder window to compare against, and opening one on the user's desktop was out of bounds.

### Privacy prompts while the research boot-args are active (`recon-pr1`; macOS behaviour)

With `amfi_get_out_of_my_way` active, every process carries `CS_PLATFORM_BINARY`, and `tccd` denied a Photos request from an app that Apple did not sign without showing a prompt: "CS_PLATFORM_BINARY set but not AppleSigned; prompt policy is Deny". A Developer ID signature made no difference. After `prepare-host --revert` and a reboot, the same app got the Contacts, Calendar, Reminders, Photos, and Automation prompts ([`demo-seeding.txt`](evidence/demo-seeding.txt)). This fits an earlier observation: this guest's grants were set up after `prepare-host`, and Recon's Screen Recording request left that pane's list empty until Recon was added with "+" ([`log.txt`](evidence/log.txt)). Recon's guide says macOS "may show its prompt" when a grant is missing; on a prepared research host, expect to grant Accessibility and Screen Recording in System Settings instead.

### Recon findings independent of virtualization

- `recon-y9v`: preflight claims a dialog-free attach (above).
- `recon-cks`: the fixture's "last ordered front is main" assumption fails on macOS 27.2.1, on the host as well.
- `recon-h3u`: inspection store records (attaches, endpoints, sessions, locks) are keyed by pid and pidversion only and survive reboots, including three "unfinished attach" records from the taskport timeouts. A new process that reuses a recorded pair would inherit them. [INFERENCE] It would then be refused with `INSPECTION_BUSY` and have no way to reconcile.
- `recon-efk`: a research session in Messages times out at `handshake` (launch) or `connect` (attach). The helper is resident, but Messages' sandbox denies its `listen` and the unlink of its endpoint socket, so Recon reports a timeout rather than the cause. Photos, Notes, Contacts, and Calendar pass ([`demo-seeding.txt`](evidence/demo-seeding.txt)).
- With `--mode launch`, an observation started too soon after the launch reports the target's own windows appearing as `UI_INTERFERENCE`. `inspect-target.sh` waits 3 s after a launch.

## Host isolation

| Check | Result |
| --- | --- |
| Host SIP, AMFI, boot-args | Unchanged at the end of the experiment |
| `prepare-host` on the host | Never run on the host |
| Clipboard, shared folders, guest app, audio | All off; no virtiofs mounts in the guest, no `/Volumes/Guest` ([`isolation-checks.txt`](evidence/isolation-checks.txt)) |
| Credentials | Dedicated ed25519 key; `ssh-add -l` in the guest finds no agent; no Apple account; the Developer ID key stays on the host; the guest password is generated and lives only in `~/ReconResearchVM/ssh` |
| Storage | The VM is in VirtualBuddy's library; its snapshots (APFS clones of the bundle, see below) are in `~/ReconResearchVM/snapshots`; none of it is tracked by git |
| Network | NAT, but **not isolated from the host or the LAN**. From the guest, host ports 22, 5000, 7000, 59917 and 62923 on 192.168.64.1, the host's LAN address, and the LAN gateway were all reachable. Loopback-only host services were not ([`guest-network-reachability.txt`](evidence/guest-network-reachability.txt)). Restricting this needs a host packet filter on `bridge100` or the host firewall, which was out of scope because it changes host configuration. |

Two host-side side effects remain:

- **VirtualBuddy deep-link approval.** VirtualBuddy now accepts deep links from `/usr/bin/open` (the Allow decision described above).
- **Stale keychain prompt.** A SecurityAgent prompt left over from the abandoned provisioning attempt stayed on the host screen. It needs a **Deny**, and the keychain item it asks about has already been deleted.

## Limitations and where the remedy belongs

| Limitation | Remedy belongs to |
| --- | --- |
| Per-boot `system.privilege.taskport` authentication before the first research attach; preflight misreports it | Recon (report or guide it); macOS configuration (authenticate once per boot, which `authorize-taskport.sh` does) |
| Host input requires VirtualBuddy to be frontmost | VirtualBuddy / Virtualization.framework |
| No modifier keys through window-addressed key events | VirtualBuddy / Virtualization.framework, or Recon's interaction event shape (not isolated) |
| VirtualBuddy provisioning sheet not fillable while secure input is on | Recon (`ui key` secure-input refusal is deliberate); VirtualBuddy (no non-GUI provisioning input) |
| Recovery, Setup Assistant, privacy grants need GUI interaction | macOS (by design); done through the VM window |
| Guest can reach host and LAN services | macOS configuration on the host (packet filter or firewall) |
| `recon:sign` needs a local signing identity | Recon tooling; worked around by host-side signing |
| No privacy prompts for apps Apple did not sign while the AMFI boot-arg is active | macOS; demo seeding reverts the boot-args first (`recon-pr1` covers Recon's guidance) |
| Research sessions in Messages time out | Recon (`recon-efk`) |

## Reusing the configured guest

A VirtualBuddy VM is one `.vbvm` bundle:

- the disk image, holding the guest's SIP policy, toolchains, Recon build and signature, staged helper, and privacy grants;
- the auxiliary storage, holding the guest NVRAM and so the boot-args;
- the machine identity, which the guest's security policy is bound to;
- VirtualBuddy's configuration.

`vm-snapshot.sh` saves the stopped bundle as an APFS clone and puts a fresh clone back in VirtualBuddy's library on request. Both take under a second, and a snapshot uses disk space only for blocks the running copy later changes. Saving `research-ready` took 32 ms and did not change the host's free space.

| Snapshot in `~/ReconResearchVM/snapshots` | State |
| --- | --- |
| `installed-unbooted` | macOS 27.2.1 installed by VirtualBuddy, never booted, configured by `configure-vm.sh` |
| `research-ready` | Everything above: SIP off, research boot-args active, Recon `549a1e4` built and signed with both privacy grants, helper staged, Xcode 27.1 and mise toolchains, CotEditor and Hex Fiend, automatic login, SSH, automatic updates off, Recon's inspection store empty |
| `demo` | `research-ready` plus the fictional persona's data in Contacts, Calendar, Reminders, Notes, Photos, Messages, and Finder ([`demo/README.md`](demo/README.md)) |

```sh
cd experiments/vm-research-environment/scripts
./vm-snapshot.sh reset research-ready   # discard the VM, start from the snapshot
./vm-lifecycle.sh boot
./authorize-taskport.sh                 # once per boot, before the first research attach
# … inspect, install, break things …
./guest-ssh.sh 'sudo shutdown -h now'   # then quit VirtualBuddy
./vm-snapshot.sh save my-setup          # optionally keep this state under a new name
```

A reset was tested end to end ([`snapshot-reset-check.txt`](evidence/snapshot-reset-check.txt)):

1. Boot from `research-ready` and write files in `~` and `/Library`, plus an NVRAM variable.
2. Shut down and reset.
3. Boot again. The files and the NVRAM variable were gone, and SIP-off, the boot-args, `readiness: eligible`, the staged helper, both privacy grants, and automatic login remained.
4. `authorize-taskport.sh` (29 s) followed by a full Calculator research inspection passed.

Constraints:

- **Quit VirtualBuddy first.** It holds library bundles open and rewrites the configuration of a loaded VM, so `vm-snapshot.sh` refuses while VirtualBuddy is running.
- **Run one copy at a time.** Every copy keeps the VM's name, MAC address, and machine identifier, and the deep links, the DHCP lease, and the guest's SIP policy depend on those.
- **Empty Recon's store before saving.** Recon's inspection records are keyed by pid and pidversion only (`recon-h3u`). Identical boots from one snapshot make a stale record's pair likely to recur, so empty `~/Library/Application Support/recon/inspection` (leave `prepare-host.lock`) before saving.
- **The taskport authorization does not survive a reboot.** It is cached for one boot, so `authorize-taskport.sh` spends the prompt on a throwaway Chess process each boot. It answers the prompt through the VM window, which brings VirtualBuddy to the front.
- **Updating Recon in a snapshot.** Build in the guest, run `sign-guest-recon.sh`, and save a new snapshot. The same Developer ID signature at the same path keeps the privacy grants. If the helper's build identity changes, rerun `prepare-host` so the staged copy matches.

## Reproducing

```sh
cd experiments/vm-research-environment/scripts
./vm-lifecycle.sh boot                  # or: recovery, stop
./guest-ssh.sh 'sw_vers'                # command in the guest
./vm-screen.sh check                    # guest display as a PNG in guest points
./security-state.sh guest label         # SIP, boot-args, prepare-host --status
./run-guest-suite.sh check 'mise run check'
./inspect-target.sh calc /System/Applications/Calculator.app attach research
./summarize-inspection.sh ~/ReconResearchVM/evidence/matrix/<run>/recon-inspect-calc
```

To recreate the guest from scratch, start from `installed-unbooted`, or install afresh:

1. Install VirtualBuddy and run the wizard with the IPSW in `_Downloads`.
2. Quit VirtualBuddy, run `configure-vm.sh`, boot, and complete Setup Assistant and Remote Login through `vm-click.sh` and `vm-input.sh`.
3. Run `provision-guest.sh BUNDLE REVISION`.
4. Run `csrutil disable` in Recovery.
5. Build with the repository tasks, then run `sign-guest-recon.sh`.
6. Grant both privacy permissions in the guest's System Settings.
7. Run `prepare-host` and reboot.
8. Save it with `vm-snapshot.sh save research-ready`.

[`evidence/log.txt`](evidence/log.txt) is the chronological record, including every dead end.
