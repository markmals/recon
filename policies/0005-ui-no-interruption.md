---
id: policy.0005
title: UI Operations Never Interrupt the User
status: active
established-by: user.2026-11-07
supersedes: [policy.0004]
---

# UI Operations Never Interrupt the User

## Introduction

Recon works on the Mac someone is using. Whatever a `ui` operation inspects or does in an application, the person at that Mac must be able to carry on with what they are doing as if Recon were not running. This policy settles that once for every `ui` operation. It states only the behaviour the person experiences; how an operation achieves it belongs to the proposal that designs the operation. Earlier records and the code call this the no-focus contract.

## The rule

Once a request's prerequisites are established, a `ui` operation must not interrupt what the person using the Mac is doing, and an operation that finds it has must fail with `UI_INTERFERENCE` rather than try to undo the interruption.

## Why this exists

An agent investigating an application is useful only if the person can keep working while it does. Bringing the application forward, using it, and putting everything back looks harmless once the command returns, but for that moment the person's typing, clicking, or place on screen was taken from them. The interruption is the failure, and undoing it is a second interruption. A desktop that looks the same afterwards is no evidence that nothing happened while the operation ran.

The previous record, [policy.0004](0004-ui-no-focus.md), stated this as a list of forbidden mechanisms. On 2026-11-07, while UI actions were being designed in [proposal.0007](../proposals/0007-macos-ui-interaction.md), that list forbade changes inside a background application that the person never experiences. The owner directed that the policy state the behaviour that matters and leave mechanisms to the designs that meet it.

## What it applies to

- Applies to: every `recon ui` operation that inspects or acts on an application, together with the work it does inside that application, from the moment the request's prerequisites are established until it returns. The person's experience is the test: what they are working in, where their typing and clicks go, where their pointer is, and which Space they are on stay theirs throughout.
- Does not apply to:
    - Changes inside the target application that the person does not experience, including the change an explicit action exists to make.
    - Changes the person makes themselves while an operation runs.
    - The setup prompts retained by [decision.0005](../decisions/0005-homebrew-ad-hoc-privacy-identity.md). A missing privacy grant may request macOS authorization and then fail with `PERMISSION_DENIED`, and `ui session open --mode attach --allow-authorization` may raise the debugger-authorization dialog.
    - `recon ui session prepare-host`, whose typed confirmation and `sudo` authentication interrupt by design.
    - `recon app launch`, an explicitly activating command advertised as one.

No exception authorizes interrupting the person to make an operation work.

## Enforcement

- Tier: task (Tier 1), with structural enforcement in the implementation.
- Mechanism: the shared guard in `crates/recon-ui-guard-macos` watches the desktop for the whole of every guarded `ui` request and turns an observed interruption into `UI_INTERFERENCE`; when it cannot watch, the request fails with `INTERFACE_UNAVAILABLE` before it runs. `crates/recon/src/observe.rs` gives every request a guard scope except host preparation, and the command registry declares both errors for exactly the guarded commands. `mise run check` runs the guard's classification and scope tests; `mise run test:focus` runs the live evidence against the signed binary and the fixture.
- Prose justification: not applicable. The live evidence is a task rather than a check because it needs a logged-in Mac with granted permissions and a desktop nobody is touching, which CI does not have.

## How to comply

1. Run every `ui` operation that inspects or acts on an application inside the shared guard; do not add a route around it.
2. Design each operation's mechanism in its proposal and show with live evidence that the person is not interrupted. This policy neither allows nor forbids a mechanism except through that evidence.
3. When an operation cannot do its work without interrupting the person, refuse with a typed reason rather than interrupting and restoring, or presenting an incomplete result as complete.
4. Exempt a new command only when it acts on no application and its interruptions are setup the operator asked for; record the exemption here first.
5. Run `mise run check`. For a change to a live path, also run `mise run test:focus` hands-off and record the evidence in the readiness report.

## Revision history

| Date | Change | Authority |
| --- | --- | --- |
| 2026-11-07 | Established, superseding policy.0004: states the behaviour the person experiences instead of the mechanisms that preserve it. | Owner direction |
