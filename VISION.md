---
title: A Vision for Recon
updated: 2026-10-27
---

# A Vision for Recon

## Introduction

Recon is an agent-facing reverse-engineering workbench for researchers and developers who need to understand software from the evidence available to them. Its scope includes binaries and recovered code, running object and view hierarchies, web documents and styles, network activity, packaged resources, and public or private platform APIs. Investigation can answer a focused question about an application's implementation, appearance, data, or behavior without requiring a recreation project.

Controlled experiments, behavioral specifications, and comparison with independent implementations remain important workflows within that scope. A useful answer says what was inspected or tried, what was found, and which evidence supports the claim, so another person or agent can challenge it. macOS is the first host; applications running on that host, in simulators, and on connected devices are distinct investigation targets.

## Current capabilities

Recon currently lists discovered and running macOS application bundles, inspects one bundle's metadata and running process IDs without activating it, and explicitly launches the selected bundle through its Rust CLI. It also observes a running macOS application without activating it, changing focus, or sending input: lists its windows, reads the accessibility tree of the application or of one window, including reachable windows on other Spaces, and captures one window to a PNG file. Those observations use Screen Recording and Accessibility permissions that belong to the locally signed `recon` executable rather than to the program that launched it.

Recon can also establish an explicit, process-bound native inspection session and read a macOS application's AppKit windows, views, cells, layers, layout, and observed fonts. Native queries under `ui view`, accessibility queries under `ui ax`, shared WindowServer capture, and native/AX correlation preserve their separate evidence sources and identities. The current native scope is Apple silicon on macOS 27; cooperative and explicitly prepared research access have different prerequisites. Catalyst, UIKit, Simulator, and connected-device inspection remain future work.

The current `ui tree` spelling is retired. Accessibility trees use `recon ui ax tree`; native hierarchies use `recon ui view` with an explicit session. Binary analysis, other runtime inspection, web and Electron inspection, live network capture, asset extraction, private-API discovery, UI interaction, experiments, behavioral specifications, verification, and persistent evidence remain future work.

Agents can learn the CLI through built-in help, inspect each command's schema, constraints, and effects, and classify compact JSON errors by stable code. Build-version and execution-context reports identify the running binary; on macOS, context checks its own signing identity and permission state without requesting grants.

Agents can list only running bundles and limit accessibility-tree depth. Application trees can also exclude the main menu bar without dropping status menus; unqualified trees still include menus. Tree nodes use one sparse encoding, and results distinguish requested narrowing from incomplete reads.

## Capability map

This map records the intended scope and its GitHub tracking. Only the current capabilities above are implemented. Open issues are capability briefs, not approved specifications, settled command syntax, or permission to change a target's security settings. Historical proposals retain the narrower intent under which their features shipped.

| Capability | Intended investigation | Tracking |
| --- | --- | --- |
| macOS applications and accessibility | Discover/inspect/launch apps; observe windows, AX trees, and screenshots. Implemented. | [#20](https://github.com/markmals/recon/issues/20), [#15](https://github.com/markmals/recon/issues/15), proposals 0001 through 0004 |
| Native binaries and managed code | Mach-O metadata, disassembly, decompilation, references, and .NET metadata/CIL. | [#2](https://github.com/markmals/recon/issues/2), [#7](https://github.com/markmals/recon/issues/7) |
| Web and Electron | Live DOM hierarchy, CSSOM rules and cascade, computed styles/layout, scripts, modules, IPC, and network observations. | [#4](https://github.com/markmals/recon/issues/4), [#5](https://github.com/markmals/recon/issues/5) |
| Native UI structure | Actual AppKit view/cell/layer hierarchies, fonts, constraints, materials, and SwiftUI backing boundaries; UIKit/Mac Catalyst hierarchy and UIKit screenshots/logs in Simulator/device investigations. | [#44](https://github.com/markmals/recon/issues/44), [#55](https://github.com/markmals/recon/issues/55), [#40](https://github.com/markmals/recon/issues/40), [#41](https://github.com/markmals/recon/issues/41) |
| Native inspection access | Establish supported, process-scoped headless sessions for macOS, UIKit Simulator, and UIKit physical-device inspectors, with explicit prerequisites and lifecycle. Mac Catalyst compatibility is verified by its inspector on the macOS session foundation. | [#51](https://github.com/markmals/recon/issues/51), [#52](https://github.com/markmals/recon/issues/52), [#53](https://github.com/markmals/recon/issues/53), [#55](https://github.com/markmals/recon/issues/55) |
| Runtime objects and interventions | Loaded classes, methods, properties, ivars, live objects/heap references; separately requested state changes and method calls. | [#45](https://github.com/markmals/recon/issues/45), [#49](https://github.com/markmals/recon/issues/49) |
| In-process network activity | FLEX-style request/response and message history inside UIKit apps in the Simulator or on connected devices, and inside AppKit apps. | [#42](https://github.com/markmals/recon/issues/42), [#43](https://github.com/markmals/recon/issues/43), [#48](https://github.com/markmals/recon/issues/48) |
| Saved network evidence | HAR/SAZ requests and exposed bodies; packet fields, filters, and summaries. These are distinct from in-process acquisition. | [#6](https://github.com/markmals/recon/issues/6), [#11](https://github.com/markmals/recon/issues/11), [#12](https://github.com/markmals/recon/issues/12) |
| Packages, images, and icons | Inventory/extract bundle resources and supported asset-catalog variants; inspect/export accessible runtime images with their different provenance. | [#13](https://github.com/markmals/recon/issues/13) |
| Platform APIs and SDK knowledge | Discover installed private-framework declarations and shared-cache metadata; separately query public SDK symbols/availability and source-attributed framework patterns. | [#46](https://github.com/markmals/recon/issues/46), [#47](https://github.com/markmals/recon/issues/47) |
| Application-local state | Bundle/container files, live and persisted preferences, SQLite evidence, and app logs, with explicit source/access boundaries. | [#3](https://github.com/markmals/recon/issues/3), [#50](https://github.com/markmals/recon/issues/50) |
| Process behavior | PTY interaction, terminal output, exit behavior, filesystem effects, and run comparisons. | [#14](https://github.com/markmals/recon/issues/14) |
| Platform control | Explicit macOS UI input; Simulator lifecycle, settings, data, and input; supported physical-device/app operations. | [#24](https://github.com/markmals/recon/issues/24), [#38](https://github.com/markmals/recon/issues/38), [#39](https://github.com/markmals/recon/issues/39) |
| Windows | Application investigation, UI observation, and explicit input under Windows-specific contracts. | [#21](https://github.com/markmals/recon/issues/21), [#16](https://github.com/markmals/recon/issues/16), [#25](https://github.com/markmals/recon/issues/25) |
| Android | Device/emulator investigation, APK code, and decoded resources. | [#9](https://github.com/markmals/recon/issues/9), [#8](https://github.com/markmals/recon/issues/8), [#10](https://github.com/markmals/recon/issues/10) |
| Investigation workflows | Bounded discovery, experiments, behavioral specification/comparison, and durable evidence across sessions. | [#17](https://github.com/markmals/recon/issues/17), [#18](https://github.com/markmals/recon/issues/18), [#19](https://github.com/markmals/recon/issues/19), [#23](https://github.com/markmals/recon/issues/23) |

In-process network inspection observes the application APIs that are instrumented, including request/response bodies those APIs expose for HTTPS. It is different from capturing encrypted packets on a host or device interface. Neither technique establishes complete coverage of every networking stack, process, or time interval. Likewise, exporting a runtime image does not recover every original resource or every compiled asset-catalog variant.

GitHub's blocking relationships record completion prerequisites; other issue cross-references can describe related capabilities or integration points without imposing whole-feature sequencing. The session issues isolate access already required by individual native inspectors. Static portions of mixed static/live issues can proceed before live-access prerequisites finish. UIKit briefs that do not yet select Simulator or physical-device support must record that choice and its session prerequisite when their proposals define supported targets.

## Prior art and intended lineage

[apple-platform-tools](https://github.com/markmals/apple-platform-tools/tree/3e587d4f0f7a5c771234fdf52ac409b6d58a65ff) supplies source-backed precedents for native AppKit/runtime inspection, Mach-O reads, private-framework header extraction, public SDK symbol queries, and curated framework-pattern search. Its `redump` disassembler work stops at backend detection, and some advertised heap/pointer-safety machinery is unfinished. SDK declarations, static framework metadata, and classes loaded in a live target answer different questions. Source inspection establishes these implementation boundaries; this alignment pass did not run or validate those tools.

[flexscope's checked-in plan](https://github.com/markmals/flexscope/tree/144d50b82462e1a328123794a21ddcce8d92e811) is specification-only and concentrates on AppKit view/runtime inspection. The owner's clarification on 2026-10-27 establishes the broader intended lineage: Flexscope was to expose functionality from [FLEX](https://github.com/FLEXTool/FLEX/tree/dac6130e0c7aca0bbf7ff8c0b57ba284776bfdff), initially through a more composable upstream library, then by rebuilding the needed functionality after the maintainer declined that direction. The checked-in Flexscope scope does not bound that intended work.

[FLEX PR #740](https://github.com/FLEXTool/FLEX/pull/740), authored by the owner, proposed a separate headless AppKit walker and is closed without merging. It explicitly excluded the core-engine split. The maintainer discussion above is owner-provided history; the public PR does not contain that discussion. Recon's direction does not depend on reviving the PR or persuading FLEX to become a composable library.

FLEX itself provides concrete UIKit prior art beyond the walker: [in-process network observation](https://github.com/FLEXTool/FLEX/blob/dac6130e0c7aca0bbf7ff8c0b57ba284776bfdff/Classes/Network/PonyDebugger/FLEXNetworkObserver.m), transaction/body recording, object/heap exploration, runtime editing and method calls, defaults, database and log inspection, [bundle/container browsing and file sharing](https://github.com/FLEXTool/FLEX/blob/dac6130e0c7aca0bbf7ff8c0b57ba284776bfdff/Classes/GlobalStateExplorers/FileBrowser/FLEXFileBrowserController.m), and [runtime image inspection/export](https://github.com/FLEXTool/FLEX/blob/dac6130e0c7aca0bbf7ff8c0b57ba284776bfdff/Classes/ObjectExplorers/Sections/Shortcuts/FLEXImageShortcuts.m). These inform native, headless Recon capabilities for UIKit and AppKit. They establish partial resource-extraction precedent, not a complete compiled-asset extractor or an existing AppKit port. Reuse must respect the relevant source licenses, including [FLEX's distribution restrictions](https://github.com/FLEXTool/FLEX/blob/dac6130e0c7aca0bbf7ff8c0b57ba284776bfdff/LICENSE).

Embedding inspection code in a cooperative app, attaching to a running app, and accessing a protected third-party or system app have different prerequisites. FLEX running inside an iOS app does not establish arbitrary-device injection; Flexscope's dedicated research-machine assumptions do not authorize changes to Recon users' machines. Each feature proposal must ground its supported targets, effects, and verification independently.

## Goals

- Make feature discovery systematic: find candidate behaviors, distinguish observations from hypotheses, and keep unanswered questions visible.
- Inspect software structure and implementation directly: code, native objects and views, DOM/CSSOM, protocols, files, assets, and platform APIs are all valid subjects of reverse engineering.
- Make focused inspection useful on its own, with machine-readable results and provenance that can later participate in a larger investigation.
- Make controlled experiments repeatable enough to isolate the effect of an input, environment change, or action sequence on the reference application's behavior.
- Turn observed behavior into precise specifications that can be tested against an independent implementation.
- Make differential verification explain agreements and disagreements with the reference, including the evidence behind each conclusion.
- Preserve observations, artifacts, conditions, and provenance across agent sessions so findings can be revisited rather than rediscovered.

## Minimum viable product

The existing minimum viable product milestone completes a bounded behavioral investigation of reference applications running on macOS through the Rust CLI: native macOS applications, Electron applications, and web applications in a browser. An agent can enumerate an application, discover multiple feature entry points and contextual prerequisites, resume its session with evidence intact, run a parameterized JSONC experiment, and turn the observations into a behavioral specification. It can execute equivalent behavior against a reference and an independent implementation, reporting matches, meaningful discrepancies, and inconclusive comparisons. SQLite stores structured records and content-addressed storage keeps larger artifacts; every claim remains traceable to observations, conditions, application versions, and the experiment that supports it.

The minimum proves the behavioral investigation loop on purpose-built macOS fixtures for native, Electron, and web applications. It is one bounded milestone within the broader reverse-engineering vision, not the definition of every useful Recon capability. Specialized inspections can deliver value before that loop is complete. Adapters and analysis providers must report unavailable capabilities and permission failures explicitly. iOS Simulator, connected iOS devices, Windows, Android, and other target environments require their own proposals and are outside this existing minimum; running their tools from a Mac does not make them macOS applications.

## Design principles

- **Evidence before confidence.** Keep raw observations, conditions, and interpretations distinguishable; a behavioral claim must point back to the experiment that supports it.
- **Control before coverage.** Prefer a small experiment with a clear baseline and variation over broad but ambiguous capture. When they conflict, reproducibility takes precedence over the number of explored features.
- **Agents can inspect the whole trail.** Favor a Rust CLI and structured, durable experiment and result records that an agent can read, write, and revisit without relying on transient chat history. Human readers need the same trail to be legible.
- **Capabilities determine integration boundaries.** Recon owns a coherent CLI, targeting, capability discovery, and evidence contracts. It can implement native inspection capabilities and use external providers where they fit. REA remains a candidate analysis provider, especially for binary and artifact workflows; it is neither a prerequisite for every inspection nor the definition of Recon's scope. Disassembler integration should reuse mature engines rather than require Recon to build a new decompiler.
- **Different evidence stays distinguishable.** An accessibility node, a native view object, a DOM node, and a CSS rule describe different aspects of an interface. SDK declarations, metadata extracted from a binary, and APIs observed in a running process likewise establish different facts. Correlate them without presenting one as another.
- **Inspection effects are explicit.** Reading files, querying another process, loading a framework, attaching a debugger, injecting code, intercepting traffic, and changing a target have different prerequisites and effects. Report those boundaries; a research tool's prior authorization to weaken security does not transfer to Recon.
- **Uncertainty stays explicit.** A mismatch is a finding to investigate, not proof that either the reference or independent implementation is wrong. Report missing evidence and environmental differences rather than manufacture agreement.

## Beyond the minimum

- Investigation of iOS apps in the Simulator and on connected physical devices, including UIKit hierarchy/runtime inspection, application data/resources, and in-process network activity. Device management and simulator lifecycle are separate capabilities with different access constraints.
- Investigation on Windows, Android, and other platforms. Each target environment needs its own grounding of installation, running instances, observation, and control.
- Broader discovery across complex third-party applications and reusable experiments spanning multiple application families, once the macOS fixture loop is reliable.
- Native, headless capabilities informed by apple-platform-tools, Flexscope's intended FLEX-derived functionality, and FLEX itself, alongside integrations with REA, re-cli, or winctl/winctl-win where suitable. Prior art informs capabilities; its command layout, GUI, dependencies, security posture, and unfinished promises are not inherited requirements.
- Authenticated remote hosts and optional MCP integration, after local CLI investigations can be reproduced reliably.

## Non-goals

- An autonomous coding agent or application generator. Recon supplies evidence and tools to an external agent rather than choosing and implementing a recreation on its behalf.
- A guarantee of recovering original source code or producing a faithful clone. Disassembly, decompilation, metadata extraction, and direct implementation inspection are in scope; their outputs must retain uncertainty and must not be presented as the original source.
- Exhaustive, unattended exploration of every application. The initial value comes from a traceable investigation of a bounded behavior, where an agent or researcher can choose and revise the question.
- Treating one observation or a passing comparison as a universal specification. Platform, version, state, and experimental conditions limit what the evidence establishes.
- Unconditional access to every target or automatic weakening of host or device protections. Unsupported attachment, unavailable inspection interfaces, and encrypted or inaccessible evidence must be reported honestly.

## Revision history

| Date | Change | Related proposal |
| --- | --- | --- |
| 2026-10-14 | Created the initial project vision. | N/A |
| 2026-10-15 | Added macOS application investigation through the Rust CLI. | proposal.0001 |
| 2026-10-15 | Limited the minimum viable product to macOS, including Electron and web applications; moved Windows and Android beyond it. | N/A |
| 2026-10-15 | Added macOS UI observation of windows, accessibility trees, and window screenshots. | proposal.0002 |
| 2026-10-27 | Added runtime command discovery, typed JSON errors, build provenance, and preflight-only execution-context reporting. | proposal.0003 |
| 2026-10-27 | Added running-only application listing, depth and main-menu narrowing, and sparse tree output with explicit completeness markers. | proposal.0004 |
| 2026-10-27 | Clarified reverse engineering across code, runtime/UI structure, web styles, network activity, resources, and platform APIs; retained behavioral verification as one workflow and distinguished the existing macOS minimum from the broader scope. | Owner clarification; no feature implementation approval |
| 2026-10-27 | Recorded FLEX-derived UIKit/AppKit network, resource, runtime, and application-state scope; distinguished the owner's intended reimplementation from the unbuilt Flexscope checkout and closed AppKit-walker PR #740; linked capability families to GitHub issues. | Owner clarification; no feature implementation approval |
| 2026-10-29 | Added process-bound native AppKit inspection sessions, native view/cell/layer/layout/font queries, the `ui ax` migration, and native/AX correlation with shared no-focus behavior. | proposal.0005 |
