---
id: policy.0001
title: CLI Command and Error Contract
status: active
established-by: proposal.0003
supersedes: []
---

# CLI Command and Error Contract

## Introduction

Recon's command discovery and failure classifications must describe the executable an agent is running. The command registry and typed failures protect that contract as commands are added.

## The rule

Define every Recon command in the command registry, and report every failure with a code from the published error enumeration.

## Why this exists

A separately maintained usage string can advertise grammar the parser does not accept. Unclassified error strings make callers infer recovery from English. One registry supplies parsing, help, and command specifications; typed error codes supply machine-readable failure classification.

## What it applies to

- Applies to: all existing and future Recon commands, including `schema` and `context`, and failures reported by the re-executing parent.
- Does not apply to: test probe executables or failures emitted by the operating system outside Recon's control. The output-delivery boundaries in [proposal 0003](../proposals/0003-agent-facing-cli-contract.md#errors) still apply when a sink fails.

Adding an error code is an ordinary contract change. Removing or renaming one changes the published contract and must be treated accordingly.

## Enforcement

- Tier: check (Tier 0), with structural enforcement in the implementation.
- Mechanism: `crates/recon/src/cli/registry.rs` supplies the parser, help, and schema metadata. `Failure` requires an `ErrorCode`; schema generation publishes the enumeration. `crates/recon/tests/cli_contract.rs` checks registry discovery, parser constraints, and declared error-code coverage. `crates/recon/tests/cli_schema.rs`, the shared error-validation helpers, and the opt-in observation tests validate actual response and error envelopes against their advertised schemas. CI runs the crate checks through `vp run check`.

These mechanisms enforce the rule; this record identifies the contract they protect without introducing a second command or error catalogue.

## How to comply

1. Add or change commands through the existing registry and native typed parsers. Derive help and command metadata from that same source.
2. Represent failures with the existing typed error contract and declare each command's possible codes. Keep the serialization schemas and behavioral coverage aligned.
3. Run `vp run check`. For a changed permission-gated observation path, also exercise the existing signed UI checks with the required grants. Revise proposal intent before changing the public contract.

## Revision history

| Date | Change | Proposal |
| --- | --- | --- |
| 2026-10-27 | Established the approved registry and published-error-code rule at completion. | proposal.0003 |
