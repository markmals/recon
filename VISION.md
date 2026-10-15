---
title: A Vision for Recon
updated: 2026-10-14
---

# A Vision for Recon

## Introduction

Recon is an agent-facing workbench for understanding how desktop applications behave when their implementation is unavailable. It is for researchers and developers who need to turn observations of macOS and Windows reference applications into reproducible behavioral specifications. A useful answer must say what was tried, what happened, and which evidence supports the claim, so another person or agent can challenge it.

## Goals

- Make feature discovery systematic: find candidate behaviors, distinguish observations from hypotheses, and keep unanswered questions visible.
- Make controlled experiments repeatable enough to isolate the effect of an input, environment change, or action sequence on the reference application's behavior.
- Turn observed behavior into precise specifications that can be tested against an independent implementation.
- Make differential verification explain agreements and disagreements with the reference, including the evidence behind each conclusion.
- Preserve observations, artifacts, conditions, and provenance across agent sessions so findings can be revisited rather than rediscovered.

## Minimum viable product

The first useful Recon completes a bounded investigation on native macOS and Windows reference applications through the same Rust CLI. An agent can enumerate an application, discover multiple feature entry points and contextual prerequisites, resume its session with evidence intact, run a parameterized JSONC experiment, and turn the observations into a behavioral specification. It can execute equivalent behavior against a reference and an independent implementation, reporting matches, meaningful discrepancies, and inconclusive comparisons. SQLite stores structured records and content-addressed storage keeps larger artifacts; every claim remains traceable to observations, conditions, application versions, and the experiment that supports it.

Native analysis through REA and supported UI adapters must report unavailable capabilities and permission failures explicitly. The minimum proves the investigation loop on purpose-built fixtures across both reference platforms; it does not imply that every feature of a complex proprietary application can already be explored automatically.

## Design principles

- **Evidence before confidence.** Keep raw observations, conditions, and interpretations distinguishable; a behavioral claim must point back to the experiment that supports it.
- **Control before coverage.** Prefer a small experiment with a clear baseline and variation over broad but ambiguous capture. When they conflict, reproducibility takes precedence over the number of explored features.
- **Agents can inspect the whole trail.** Favor a Rust CLI and structured, durable experiment and result records that an agent can read, write, and revisit without relying on transient chat history. Human readers need the same trail to be legible.
- **Analysis and orchestration have separate owners.** REA is the primary reverse-engineering analysis backend; Recon owns the investigation workflow, behavioral records, and comparison of evidence. An integration must preserve that boundary rather than duplicate REA's analysis.
- **Uncertainty stays explicit.** A mismatch is a finding to investigate, not proof that either the reference or independent implementation is wrong. Report missing evidence and environmental differences rather than manufacture agreement.

## Beyond the minimum

- Broader discovery across complex third-party applications and reusable experiments spanning multiple application families, once the cross-platform fixture loop is reliable.
- Integrations with tools such as re-cli or winctl/winctl-win where they provide real analysis or application control capabilities; the boundaries and suitability of each require separate evaluation.
- Authenticated remote hosts and optional MCP integration, after local CLI investigations can be reproduced reliably.

## Non-goals

- An autonomous coding agent or application generator. Recon supplies evidence and tools to an external agent rather than choosing and implementing a recreation on its behalf.
- Source-code recovery or a faithful clone of an application. The target is externally observable behavior; implementation details are only relevant when they help explain or verify it.
- Exhaustive, unattended exploration of every desktop app. The initial value comes from a traceable investigation of a bounded behavior, where an agent or researcher can choose and revise the question.
- Treating one observation or a passing comparison as a universal specification. Platform, version, state, and experimental conditions limit what the evidence establishes.
- Replacing REA as a reverse-engineering analysis backend. A second analysis engine would blur responsibility before the evidence workflow is proven.

## Revision history

| Date       | Change                              | Related proposal |
| ---------- | ----------------------------------- | ---------------- |
| 2026-10-14 | Created the initial project vision. | N/A              |
