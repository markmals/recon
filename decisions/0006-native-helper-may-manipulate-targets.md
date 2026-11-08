---
id: decision.0006
title: Native Helper May Manipulate Targets
status: accepted
established-by: user.2026-11-07
supersedes: [decision.0003]
---

# Native Helper May Manipulate Targets

## Summary

Withdraws decision.0003. Recon's native helper may manipulate the application it runs in as well as inspect it. What each helper operation does is decided by the proposal that adds it, not by a standing record of the helper's current scope.

## Context

[decision.0003](0003-recon-owned-native-loader.md) recorded the native loader and helper as [proposal.0005](../proposals/0005-native-macos-inspection.md) built them, including that "the helper exposes only authenticated, bounded inspection operations". That sentence described the tool when it was written. [proposal.0007](../proposals/0007-macos-ui-interaction.md) then needed the helper to make a Mac Catalyst window key inside UIKit, because Catalyst ignores key chords until UIKit has a key window and nothing outside the process could set it. Read as a decision, the record forbade any helper operation that changes the target.

## Decision

Recon does not limit its native helper to inspection. The helper may inspect and manipulate the application it runs in. Each operation, and how the loader and helper deliver it, is specified and approved by the proposal that adds or changes it, and is held to the active policies, including [policy.0005](../policies/0005-ui-no-interruption.md) and [policy.0001](../policies/0001-cli-command-and-error-contract.md). decision.0003 no longer constrains the loader or the helper; their current design is described by proposal.0005 and the code.

## Rationale

A record that describes a component's current scope turns every extension of it into a conflict with a commitment nobody made. Whether a given manipulation is appropriate depends on what it does, which only the proposal that adds it can show. The owner considers manipulating the target squarely within the helper's purpose.

## Alternatives considered

- Revise decision.0003 to allow one interaction operation for Catalyst key chords — keeps the inspection-only framing and makes every later manipulation another exception.
- Replace decision.0003 with a record that keeps its loader guarantees as standing constraints — the owner chose to withdraw the record rather than preserve parts of it as intent.

## Consequences

### Good

- Helper operations that change the target, starting with proposal.0007's Catalyst key-window operation, are proposed and reviewed on their merits.
- Records stop presenting the current implementation as intent.

### Bad

- No standing record bounds what the helper may do inside a target; each proposal that extends it must justify the authority it adds, and its review must check that justification.
- The loader's incarnation checks and the helper's authenticated, bounded protocol are no longer protected by a decision record, so a proposal could change them without a record conflict, and reviews must notice when one does.

## Revision history

| Date       | Change                                               | Authority       |
| ---------- | ---------------------------------------------------- | --------------- |
| 2026-11-07 | Accepted, superseding and withdrawing decision.0003. | Owner direction |
