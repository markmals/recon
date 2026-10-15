---
id: proposal.0001
title: macOS Application Investigation
authors: [Mark Malstrom]
status: draft
pull-request: https://github.com/markmals/recon/pull/1
issues: []
supersedes: []
---

# macOS Application Investigation

## Summary

Add the first Recon CLI feature: list discoverable macOS applications, inspect an application's identity and bundle metadata, and launch a selected application. The commands give later UI observation and experiments an unambiguous target without claiming to discover behavior or analyze binaries.

## Authorization

- Authority: Mark consented to a first feature proposal for macOS application investigation in the conversation on 2026-10-14, after reviewing the proposed scope of `recon app list`, `recon app inspect`, and `recon app launch`.
- Fit: This proposal is limited to those three commands on the available macOS host. No build mandate applies.
- Decisions recorded: None yet. The proposal remains a draft for human review; consent to propose is not approval to implement.

## Motivation

Recon's proposed UI and experiment commands name an application, but there is currently no way for an agent to find a candidate, tell which installed copy it means, or open that copy through Recon. The workbench has no Recon executable or application adapter. An agent must instead use unrelated operating-system tools and carry the target identity between steps without a shared CLI contract. A display name such as `Logic Pro` is convenient for a person but is not sufficient when multiple applications have the same name or bundle identifier.

## Domain grounding

### Established context

- macOS distinguishes an application bundle at a filesystem URL from a running instance. A bundle has metadata such as its identifier and display name; a running application has a process identifier and changing state. See Apple's [bundle identifier documentation](https://developer.apple.com/documentation/bundleresources/information-property-list/cfbundleidentifier) and [NSRunningApplication documentation](https://developer.apple.com/documentation/appkit/nsrunningapplication).
- Launch Services registers applications known to the system, but registration and application files are not identical inventories. A user may have copies of an app, and a display name is not a unique selector. Apple's [Launch Services guide](https://developer.apple.com/library/archive/documentation/Carbon/Conceptual/LaunchServicesConcepts/LSCConcepts/LSCConcepts.html) describes registration, bundle metadata, and launch/reopen behavior.
- Apple's [file metadata query API](https://developer.apple.com/documentation/foundation/nsmetadataquery) discovers matching indexed files and gathers results asynchronously. Discovery through the system's application search is therefore not proof that every application on every volume was enumerated. Apple's [modern workspace API](<https://developer.apple.com/documentation/appkit/nsworkspace/openapplication(at:configuration:completionhandler:)>) opens an application at a specific URL and reports success or failure; its [configuration](https://developer.apple.com/documentation/appkit/nsworkspace/openconfiguration) controls activation, substitution of a running instance, and creation of a new instance.

### Relevant constraints and principles

- Repository evidence: [VISION.md](../VISION.md) calls for a Rust CLI that can enumerate an application on macOS and Windows, but the current `Cargo.toml` has no workspace members, `crates/` is empty, and the existing CI checks only Ubuntu. The initial feature is macOS-only, not a claim that the cross-platform MVP is done.
- Repository evidence: the vision assigns reverse-engineering analysis to REA. Finding and opening an application must not introduce a competing native-analysis engine or require a configured Hopper, IDA, or Ghidra provider.
- Inference: choosing a concrete bundle location before launch will keep later UI operations attached to the intended installed copy rather than silently choosing a preferred application with the same name or identifier.

### Quality bar

- A person or agent can distinguish each listed copy by a concrete location, inspect that same copy, and launch it without an implicit switch to another copy.
- An empty or incomplete discovery result is not presented as an exhaustive inventory. Missing, ambiguous, unreadable, or unlaunchable targets produce actionable diagnostics rather than an arbitrary selection or silent success.
- Read-only commands never open the target; the launch command is an explicit, visible side effect and does not claim that a window or UI element is ready.

### Remaining uncertainty

- The exact coverage of application discovery on a given host depends on the system's indexed and accessible locations; the CLI must describe its discovery boundary rather than promise a complete filesystem census.

## Existing baseline

The repository contains development tooling, process documents, and a documentation site but no Recon CLI, macOS adapter, or app-investigation tests. macOS itself supplies bundle metadata, application discovery, running-instance information, and launch services; those platform facilities are the usable baseline, not a Recon implementation. The docs preview workflow previews the project record, not an installable CLI, so it cannot exercise this feature.

## Proposed solution

Provide a small Rust CLI with an `app` command group for macOS. `list` reveals applications the host can discover, `inspect` reports the metadata for one selected application bundle, and `launch` opens that exact bundle through macOS and reports the resulting process. All three share the same targeting rules.

```text
recon app list → discoverable bundles with names, identifiers, and paths
recon app inspect "Logic Pro" → metadata for exactly one matching bundle, or an ambiguity error
recon app launch /Applications/Logic\ Pro.app → open that bundle and report its process, or explain the failure
```

## Detailed design

### Targets and discovery

- A target is an installed macOS application bundle at a filesystem location, not its display name, bundle identifier, or running process. Commands accepting a target accept an absolute `.app` path, an exact displayed name, or an exact bundle identifier. Bundle identifiers match case-insensitively as on macOS. A path can select a valid application bundle that `app list` did not discover.
- Name and identifier lookup use the host's discoverable application set. If a string matches both a name and an identifier, all matching bundles are considered. When lookup has zero matches, report not found; when it has multiple bundle locations, report ambiguity with each candidate's name, identifier when available, and path. Never select an arbitrary first match, including when two installed copies share an identifier. An absolute path identifies one bundle and does not silently resolve to another copy.
- `recon app list` reports discoverable application bundles, including separate locations for multiple copies. It sorts by displayed name and then absolute path, and includes each bundle's name, bundle identifier when present, and path. It does not claim to enumerate unindexed, inaccessible, or disconnected volumes. If discovery itself is unavailable, return an explicit error rather than an apparently authoritative empty list.
- `recon app inspect <target>` reports the resolved absolute path, displayed name, bundle identifier when present, version and build identifiers when available, and executable path when available. Missing optional bundle metadata is reported as absent, not fabricated. An unreadable, non-application, or no-longer-existing path is an error. Inspection does not start or activate the application.

### Launch and output

- `recon app launch <target>` resolves the target under the same rules, then asks macOS to open the selected bundle. It may activate an already running instance of that exact bundle, but does not request an additional instance or substitute a running copy at another path. On success it reports the bundle path and the process identifier returned by the host; success means the host opened or activated the app, not that its UI is ready for interaction.
- Launch does not terminate an existing process, pass user-supplied launch arguments, change application files, or attempt to bypass macOS security prompts. A refusal, missing target, terminated or unlaunchable app, or host launch failure exits unsuccessfully with an actionable diagnostic. `list` and `inspect` have no launch side effects.
- Successful commands write one JSON object to standard output: `list` returns `{"discovery":"indexed","applications":[{"name":string,"bundleId":string|null,"path":string},...]}`; `inspect` returns `{"application":{"name":string,"bundleId":string|null,"path":string,"version":string|null,"build":string|null,"executablePath":string|null}}`; `launch` returns `{"path":string,"pid":integer}`. Paths are absolute application-bundle or executable paths as named. The displayed name falls back to the bundle filename when macOS has no display name; nullable fields represent unavailable metadata. Errors go to standard error with a nonzero exit status and no success JSON. Human readers can inspect the same JSON without a separate output mode.
- On a host other than macOS, these commands explicitly report that this implementation does not support that host. They must not pretend an empty application list or a successful no-op launch. This proposal does not supply a Windows adapter.

## Compatibility

No compatibility impact: Recon currently has no CLI, callers, or stored application records to migrate. The commands introduce an initial interface whose selectors and output fields will be tested and documented before implementation.

## Implications on adoption

Users need macOS and an installable Rust-built Recon executable. `list` and lookup depend on applications being discoverable by the host's application search; explicit bundle paths allow inspection and launch of a valid app outside that discovery set. `launch` can run or foreground software with the current user's permissions, so users should target applications they intend to open. No REA installation or licensed native-analysis provider is required.

## Scope

- Build the initial Rust CLI with the three `recon app` commands, macOS application discovery/metadata/launch integration, shared unambiguous target resolution, and useful errors.
- Add behavior-focused tests for duplicate names or identifiers, path selection, unavailable discovery, absent metadata, non-app targets, and launch outcomes; exercise the real commands on macOS with an actual application fixture.
- Write a CLI guide explaining selectors, output, discovery limits, and launch side effects before production code. Provide macOS validation and an installable macOS executable as the PR preview; the existing Ubuntu project checks remain required.

### Out of scope

- UI trees, screenshots, accessibility permissions, click/key interaction, and automatic feature discovery: these need their own observable UI feature and may build on the target contract here.
- Binary inspection and REA integration: application metadata does not require a native-analysis provider.
- Experiment execution, specification generation, verification, and persistent evidence storage: these follow later feature proposals rather than becoming incidental requirements of listing or launching.
- Windows application investigation, a complete census of every filesystem volume, signing/notarization, and automatic launching during `list` or `inspect`: none is needed to validate this macOS feature.

## Preview

- Artifact: an installable macOS CLI executable made available through the draft PR, with a macOS application fixture or instructions for a built-in app. The reviewer installs and exercises `list`, `inspect`, and `launch` on the Mac itself; an Ubuntu build or a rendered documentation page is not a substitute.
- Reason: it is the cheapest realistic way to observe actual application discovery and launch behavior, including ambiguity and macOS failures. Keep the executable as a prerelease PR artifact with normal artifact expiration; do not publish or install it as a production release.

## Policies and decisions checked

None. `policies/` and `decisions/` currently contain only README files and no governing numbered records. The proposal follows the current vision's Rust CLI and explicit-capability principles without claiming to satisfy the Windows portion of the MVP.

## Future directions

- A Windows application adapter could implement the same user-facing targeting contract, after grounding the Windows distinction between installed apps and running processes.
- UI observation and interaction could accept the selected bundle or running-instance identity without treating a display name as a unique key.
- Later investigation records could capture the exact bundle location, version, and observed process of a run rather than use the current display name as historical evidence.

## Alternatives considered

- Start with the experiment runner: rejected for proposal one because it needs reliable application targeting and UI control and would silently bundle several feature areas.
- Use only display names or choose the first bundle-identifier match: smaller syntax, but duplicate installations and localized names can make it open the wrong application without warning.
- Search only fixed `/Applications` directories: easy to implement but misses apps in user directories or other indexed locations. System application discovery with explicit-path access is more useful, while its incomplete coverage remains visible.
- Integrate REA before listing applications: adds an external native-analysis prerequisite to a feature that requires only OS application metadata and launch behavior.

## Open questions

None at draft time. The human may revise discovery coverage, target syntax, and launch behavior during proposal review; this document does not authorize implementation.
