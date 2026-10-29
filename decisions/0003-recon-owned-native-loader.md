---
id: decision.0003
title: Recon-Owned Incarnation-Bound Native Loader
status: accepted
established-by: proposal.0005
supersedes: []
---

# Recon-Owned Incarnation-Bound Native Loader

## Summary

Recon uses its own universal C loader and matching helper for native macOS inspection. The loader verifies the selected process incarnation before every target-side effect, and the helper exposes only authenticated, bounded inspection operations.

## Context

Native AppKit inspection requires target-side execution on the application's main thread. A debugger or general-purpose injection framework would add a broad runtime and a second security model. A PID alone is not a process identity: it can be reused between selection and injection. Apple-silicon targets also require both arm64 and arm64e support, pointer-authentication-aware thread setup, and an honest research-host boundary for protected applications. Prior art supplied useful mechanisms, but not Recon's identity, lifecycle, or delivery guarantees.

## Decision

Build `recon-loader` for arm64 and arm64e, with Recon's own debugger and thread-state entitlements. It acquires a task control port, verifies the audit token's PID and pidversion, checks dyld shared-cache agreement, and revalidates that incarnation before allocation and again before starting the injected thread. Its bootstrap enters only `recon_helper_main` in the matching Recon helper.

The helper uses an owner-private local socket, kernel peer audit tokens, per-session capabilities, and a versioned, size-bounded protocol. It supports closed inspection operations rather than arbitrary code evaluation. Helper build mismatches fail instead of triggering replacement or downgrade. First-party and other non-cooperative targets require the separately prepared research profile; the loader never weakens host or target protections as an inspection side effect.

## Rationale

Owning this narrow path makes the pre-effect identity guarantee, arm64e behavior, protocol bounds, and cleanup semantics directly testable. A C loader avoids the unsupported arm64e Rust runtime failure observed during implementation. A closed helper keeps the privileged target-side surface smaller than a general debugger or scripting endpoint. Explicit build and host checks preserve the distinction between cooperative inspection and an operator-weakened research machine.

## Alternatives considered

- Frida or another maintained injection framework — rejected by the project owner; it would introduce a second runtime and trust model rather than Recon-owned identity and protocol guarantees.
- A Rust-only loader — simpler integration, but the arm64e standard-library initialization crashed before `main` on the supported toolchain, so it could not meet the required target matrix.
- Cooperative embedding only — avoids protected-target complexity but excludes the required research-machine outcome.
- Unchecked PID-based injection or a general debugging endpoint — easier to implement, but cannot prevent effects on a replacement process and unnecessarily expands target-side authority.

## Consequences

### Good

- Every target-side effect is tied to a kernel-verified process incarnation.
- One loader/helper pair covers arm64 and arm64e with explicit PAC handling and repeatable fixture tests.
- Resident code, session access, helper build compatibility, and host preparation remain separate, observable concepts.
- The target-side API stays bounded to Recon's inspection operations.

### Bad

- Recon owns Mach, dyld, pointer-authentication, and macOS-version compatibility maintenance.
- Closing a session cannot safely unload the helper; full replacement requires target restart.
- First-party inspection requires an explicitly weakened research host and is not a stock-machine capability.
- Packaging must keep the CLI, loader, helper, and build identity together.

## Revision history

| Date | Change | Proposal |
| --- | --- | --- |
| 2026-10-28 | Accepted the Recon-owned, incarnation-bound loader and closed helper protocol. | proposal.0005 |
