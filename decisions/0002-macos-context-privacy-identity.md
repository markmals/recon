---
id: decision.0002
title: Recon Owns Its macOS Privacy Identity, And Can Report It
status: superseded
established-by: proposal.0003
supersedes: [decision.0001]
---

# Recon Owns Its macOS Privacy Identity, And Can Report It

## Summary

macOS privacy grants belong to the `recon` executable itself, identified as `me.malstrom.recon`, exactly as decision 0001 established. This decision restates that arrangement and extends it in one direction: a command re-executes whenever its result depends on Recon's own privacy identity, and a command may report permission state without asking macOS to request anything.

## Context

Decision 0001 settled where a grant lives. Screen Recording and Accessibility are attributed to a _responsible process_, which for a command-line tool is the terminal, editor, or harness that launched it. Recon re-executes itself once with the private `responsibility_spawnattrs_setdisclaim` spawn attribute so that its grants belong to the `recon` executable and mean the same thing under every launcher, and it is signed locally with a Developer ID identity so a grant survives a rebuild. None of that changes here, and the reasoning behind it is preserved below.

What decision 0001 did not anticipate is a command whose subject is Recon. Proposal 0003 adds `recon context`, which answers "what am I holding, and what can it do here?" — version, signing identity, permissions held, private interfaces available, and whether application discovery works. Two of its sentences stand in the way of that command.

The first is "Commands that need no permission do not re-execute". `recon context` needs no permission, and must re-execute anyway: a permission check run under the launcher's identity reports the launcher's grants, which is a different question from the one the command was asked. Reporting the terminal's Screen Recording grant as Recon's would be worse than reporting nothing.

The second is that Recon "checks a permission without prompting, asks macOS to request it when it is missing, and then fails". `recon context` checks without prompting and then neither requests nor fails. Requesting is the one thing it must never do: `CGRequestScreenCaptureAccess` may put a dialog on a screen the agent running Recon cannot see, and a report that changes the state it reports is not a report. Failing would be no better, because a missing permission is one of the answers the command exists to give.

Every claim Recon makes is supposed to be traceable to the conditions that produced it, and the permissions an observation ran under are part of those conditions. Learning them must cost one command and change nothing.

## Decision

- Recon's privacy identifier is `me.malstrom.recon`, under Mark's `me.malstrom.*` namespace for `malstrom.me`. The `recon` executable embeds an Info.plist in its `__TEXT,__info_plist` section with that `CFBundleIdentifier` and the `CFBundleName` `recon`, and its code-signing identifier is the same string.
- A command re-executes when its result depends on Recon's own privacy identity, whether because it needs a permission or because it reports Recon's permission state. Other commands do not re-execute. If macOS reports another process as responsible, `recon` re-executes itself once with the disclaim spawn attribute and passes on its arguments, environment, and standard input. The parent owns both child output streams, drains them concurrently, and withholds them until a successful wait; with successful sink writes, it then forwards each stream's bytes unchanged and preserves the child's exit status. Delivery failures follow proposal 0003's error-ownership rules. Signal forwarding and re-raising are unchanged.
- A command that needs a permission checks it without prompting, asks macOS to request it when it is missing, and then fails with an error naming the permission, the executable's path, and the System Settings pane. If the re-execution is impossible, such a command fails rather than running under the launcher's permissions.
- A command may report permission state without requesting it. `recon context` checks each permission with preflight calls only (`CGPreflightScreenCaptureAccess` and `AXIsProcessTrusted`) and succeeds whether or not a permission is held. An identity failure before spawning produces a report with `responsibleForSelf: false` and unknown grants; other commands fail rather than using the launcher's permissions. A genuine wait failure after spawning is fatal for every command, including `context`: the parent discards child output and emits only one `IDENTITY_UNAVAILABLE` error with stage `wait`, exit 1, and empty standard output. No fallback context report follows a post-spawn wait failure.
- Recon never writes to the TCC database or works around a missing grant.
- `recon` ships as a bare executable, not an application bundle. Users sign it locally with a Developer ID Application identity and the identifier `me.malstrom.recon` (`codesign --force --sign "<identity>" --identifier me.malstrom.recon <path>`). Contributors use `RECON_SIGNING_IDENTITY=… vp run recon:sign`. CI builds stay ad-hoc signed until Recon has a distribution channel, such as a Homebrew formula, that brings signing into CI. That change revises this decision.

## Rationale

Owning the identity is the only arrangement under which a grant means the same thing in every terminal and agent harness. Recon's evidence depends on observations that can be repeated, so a grant that varies with the launcher would undermine every result built on it. That argument is what makes the extension necessary rather than convenient: a report of Recon's permissions is worthless under the launcher's identity for the same reason an observation would be, so the rule about re-execution follows the dependency on identity rather than the need for a permission.

Reporting without requesting lets an agent learn permission state through preflight calls without changing grants. An identity failure before spawning is one of the conditions `context` exists to explain, so its report carries `responsibleForSelf: false` and `granted: null` rather than claiming the launcher's grants as Recon's. Once a child exists, the parent must own the final response: withholding both streams until a successful wait prevents a child response and a parent wait-failure response from both escaping. Mark approved this distinction on 2026-10-26 in the proposal's recorded authorization.

An embedded Info.plist gives the bare executable a recognizable name in System Settings without the packaging and launch semantics of an app bundle. The bare form is also the one verified to prompt and keep its grant. A Developer ID signature is the cheapest signature whose designated requirement survives a rebuild. Local signing keeps a certificate out of CI until a distribution channel needs one.

The identifier follows the owner's existing reverse-DNS convention, because any later change costs every user a fresh grant.

## Alternatives considered

- **Leave decision 0001 as it stands and let `recon context` read the launcher's permissions.** It needs no re-execution and no new rule, but the answer would describe the terminal rather than Recon, which is the failure decision 0001 exists to prevent.
- **Report permissions only when the self-disclaim succeeds, and fail otherwise.** This keeps the old "fails rather than running under the launcher's permissions" sentence intact for every command, but it withholds the report precisely when something is wrong with the identity — the case the command is most needed for.
- **Report `granted: false` when Recon is not responsible for itself.** It avoids a third value in the field, but it states something Recon did not observe: the preflight answered for the launcher.
- **Let `recon context` request a permission it finds missing, as `ui` commands do.** It would make the command a one-stop fix, but a report that puts a dialog on a screen the agent cannot see is not read-only, and the request would change the state the next run reports.
- **Keep both rules by giving `recon context` a `--no-request` option.** Rejected: an agent with no prior knowledge of Recon cannot opt into a flag it does not know exists, and a command that sometimes shows a dialog cannot be described as safe.
- **Rewrite decision 0001 in place.** Rejected by the repository's own convention: history is preserved, and a superseded decision keeps its text and gains a replacement.

## Consequences

### Good

- A permission grant behaves the same whichever program launches `recon`, including cron jobs and agent harnesses.
- One grant per installed executable survives in-place upgrades signed with the same identifier and team.
- An agent can learn Recon's version, identity, permissions, private interfaces, and discovery availability in one command that changes nothing and can show no dialog.
- A failed self-disclaim is diagnosable: it is reported with its reason instead of becoming a command that cannot run.
- Interaction (#24) and later UI features inherit the identity, re-execution, and permission errors instead of solving them again.
- A missing permission is reported with an actionable path and pane, never hidden behind empty or partial results.

### Bad

- Recon depends on the private `responsibility_get_pid_responsible_for_pid` and `responsibility_spawnattrs_setdisclaim` interfaces. A macOS update can remove them, and Recon then reports the capability as unavailable.
- `recon context` re-executes itself, so the cheapest command Recon has still costs two processes.
- Grants are tied to a path. Moving the executable, or running a second copy elsewhere, needs a new grant.
- Keeping grants across rebuilds requires a Developer ID certificate. Without one, a user must grant again after every build.
- CI downloads must be re-signed before use until CI signing exists.
- The identifier must never change casually. Superseding it invalidates every existing grant.
- Since macOS 15, macOS periodically asks users to reconfirm screen-recording permission, which can interrupt unattended use.
- Apple states the non-prompting behavior of `CGPreflightScreenCaptureAccess` in forum answers rather than reference documentation, and steers screen-capture clients toward ScreenCaptureKit. If a later macOS starts noticing repeated preflights, `recon context` is where that would show up first.

## Revision history

| Date | Change | Proposal |
| --- | --- | --- |
| 2026-10-15 | Accepted decision 0001, which this decision supersedes. | proposal.0002 |
| 2026-10-26 | Re-execution follows dependence on Recon's identity; permission state may be reported without being requested. | proposal.0003 |
