---
id: policy.0004
title: One No-Focus Contract for UI Operations
status: superseded
established-by: user.2026-11-02
supersedes: [policy.0003]
---

# One No-Focus Contract for UI Operations

## Introduction

Recon inspects applications on the Mac someone is using. Every mechanism it reaches for — accessibility, the window server, a loaded helper, a future UI action — has a shortcut available: bring the target forward, read what is now in front, and put things back. This policy settles that recurring decision once, for every `ui` operation that inspects or acts on an application, instead of leaving each backend to answer it.

## The rule

Once a request's prerequisites are established, no `ui` operation may activate or raise the target, transfer keyboard focus, make a window key, switch Spaces, move the real pointer, or divert input; an operation that observes such a disturbance must fail with `UI_INTERFERENCE` rather than restore what it saw change.

## Why this exists

The behavior is unenforceable as an intention. "Focus without raise" still takes the keyboard away from whatever the user is typing into; activate-then-restore, a cursor moved and moved back, and a Space switched to reach a window all leave the desktop looking untouched once the command returns. The [winctl hittest research](https://github.com/NSExceptional/winctl/blob/295a02751b2cb0c958b476e6ce6864963ee4042f/research/hittest/README.md) that Recon's UI work derives from tested exactly those alternatives; they are plausible, they work, and they are indistinguishable from a clean read if the only evidence is the state before and after.

So the rule has a second half. A disturbance is a failed operation with observed effects, reported through `UI_INTERFERENCE` with its stage, evidence source, affected process and window, and whether target execution may continue. Taking focus back would be a second disturbance, and a quiet desktop at the end is not proof that the desktop was quiet throughout. Recon also does not blame what it cannot attribute: a change observed while the hardware was generating input belongs to the person at the keyboard, not to the target.

This record supersedes [policy.0003](0003-ui-no-focus.md), which applied the contract to every operation reachable through `recon ui`. That scope reached `recon ui session prepare-host`, which inspects nothing: on 2026-11-02 its own `sudo` Touch ID dialog came forward during an apply that had already succeeded, and the command reported `UI_INTERFERENCE`. The owner directed that host preparation be exempt.

## What it applies to

- Applies to: every `recon ui` operation that inspects or acts on an application — window inventory, `ui ax`, `ui view`, `ui session preflight`, `open` including background launch, and `close`, screenshots, and `ui correlate` — together with the target-side work they dispatch, and the UI actions proposed in [proposal 0005](../proposals/0005-native-macos-inspection.md) issue #24. A supported action changes the target's own responder state; it still may not move the user's system focus.
- Does not apply to:
    - The setup prompts retained by [decision 0002](../decisions/0002-macos-context-privacy-identity.md). A missing privacy grant may request macOS authorization and then fail with `PERMISSION_DENIED`, which is reported even when the request dialog came forward while the operation ran. `ui session open --mode attach --allow-authorization` may raise the debugger-authorization dialog. Those dialogs interrupt the desktop by design, which is why the no-interruption guarantee is conditional on prerequisites already being established.
    - `recon ui session prepare-host`, in all its forms. It prepares the host rather than inspecting an application: it has no target, reads no window or element, and its typed confirmation and `sudo` authentication, by password or Touch ID, interrupt the desktop by design.
    - `recon app launch`, which is an explicitly activating command and is advertised as one.

Neither exception authorizes activating a target to make an observation work, and a request whose prerequisites are present has no exception left to claim.

## Enforcement

- Tier: task (Tier 1), with structural enforcement in the implementation.
- Mechanism: `crates/recon-ui-guard-macos/src/focus.rs` watches the frontmost application, the order of the on-screen windows, the active Space, and the pointer and input event streams about every ten milliseconds, for the whole of every guarded `ui` request, and turns an observed disturbance into `UI_INTERFERENCE` instead of a success. A dimension this Mac cannot report fails the operation with `INTERFACE_UNAVAILABLE` before it runs. `crates/recon/src/observe.rs` gives every request a guard scope except host preparation, and the command registry declares `UI_INTERFERENCE` and `INTERFACE_UNAVAILABLE` for exactly the guarded commands. `mise run check` runs the guard's classification tests, the scope and published-error tests, and the continuous-watch coverage test in `crates/recon/tests/ui_focus.rs`. `mise run test:focus` runs the live evidence out of process against the signed binary and the fixture; `cargo test -p recon --test ui_focus an_activating -- --ignored` exercises the counter-case, where an explicitly activating command must be visible to the same watch.
- Prose justification: not applicable. The live evidence is a task rather than a check because it needs a logged-in Mac with the permissions granted to one signed executable and a desktop nobody is touching, which CI does not have; the in-process guard and its classification tests run unconditionally.

## How to comply

1. Reach macOS through the guarded path. A new `ui` operation that inspects or acts on an application runs inside `guarded` with the stage and selected instance it is about; do not add a route that reaches the window server, accessibility, or a helper around it.
2. When a mechanism cannot do the requested work inside the boundary, report that with its typed reason — an unavailable interface or an unavailable inspection — rather than degrading to activation, a retry that takes focus, or an empty result presented as complete.
3. Exempt a new command only when it inspects and acts on no application, and its interruptions are setup the operator asked for; record the exemption here first.
4. Run `mise run check`. For a change to a permission-gated or live path, also run `mise run test:focus` hands-off and record the evidence in the readiness report. Revise proposal intent before changing what the contract promises.

## Revision history

| Date | Change | Authority |
| --- | --- | --- |
| 2026-10-28 | Established the cross-command no-focus rule and its guard at completion (policy.0003). | proposal.0005 |
| 2026-11-02 | Exempted `ui session prepare-host`, and made a failed permission request report `PERMISSION_DENIED` rather than its dialog. | Owner direction |
