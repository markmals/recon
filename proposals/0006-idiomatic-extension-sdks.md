---
id: proposal.0006
title: Idiomatic Extension SDKs
authors: [Mark Malstrom, OpenAI Codex]
status: draft
pull-request: https://github.com/markmals/recon/pull/82
issues: ["https://github.com/markmals/recon/issues/74"]
supersedes: []
---

# Idiomatic Extension SDKs

## Summary

Restructure Recon into three Rust workspace crates: the `recon` CLI, a platform-independent `recon-core`, and macOS integration in `recon-macos`. Introduce a purpose-designed Swift extension SDK whose macros export author-defined named actions with typed inputs and results, rather than requiring a fixed native-inspection protocol. Prove the architecture with the AppKit inspector while preserving existing command, access, identity, and observation contracts.

## Motivation

Recon's owner wants inspection capabilities to be implemented in the language that best fits the inspected technology, starting with Swift for AppKit. Extension authors should spend their effort implementing inspection, not managing a Rust ABI, translating foreign types, or maintaining registration and serialization glue. They must be able to export a new named action without extending a central SDK protocol that enumerates all possible inspection operations.

The current Rust executable, Objective-C helper, and C loader already cross language and process boundaries, but those boundaries are private implementation details. There is no supported extension-authoring experience. Adding another inspection technology requires understanding Recon's internals and deciding anew how native functionality reaches the command surface.

Issue [#74](https://github.com/markmals/recon/issues/74) proposes porting the native helper to Rust. Language uniformity could simplify parts of maintenance, but does not answer how different inspection technologies should integrate. Conversely, changing the executable's language would not by itself produce a reusable extension contract. The problem is the missing separation between CLI, shared investigation logic, platform integration, and extension authoring, not the language of `main`.

## Domain grounding

### Established context

- [Node-API](https://nodejs.org/api/n-api.html) is a deliberately designed native-addon API with binary compatibility guarantees across Node versions. Its separation of an authoring interface from implementation details is relevant; its independent binary-compatibility commitment is not a prerequisite for Recon.
- [UniFFI](https://mozilla.github.io/uniffi-rs/latest/) generates foreign-language bindings to Rust libraries. Binding generation can implement a bridge, but does not establish an idiomatic extension API or its lifecycle semantics.
- Swift's [attached macros](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0389-attached-macros.md) can generate wrapper functions, peer declarations, and members from annotated declarations. [Swift Package Manager supports macro targets](https://www.swift.org/blog/swift-5.9-released/#swift-package-manager). This supports explicitly exported named methods without requiring an SDK protocol for every possible action. Macro expansion works from syntax; generated code must still type-check, preserve actor isolation, and use supported value encodings. Macros do not inherently provide runtime reflection, another process's main thread, or a security boundary.
- Apple's [thread-safety guidance](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html) constrains AppKit view access. The existing inspector runs in the target process and dispatches native reads to that application's main thread. A Swift host executable or host-side `@MainActor` annotation cannot replace target-side execution.
- Apple explicitly states that [Objective-C exceptions must be caught before reaching Swift code](https://developer.apple.com/documentation/swift/handling-cocoa-errors-in-swift). Swift `throws` is not an Objective-C exception boundary. Wrapping a Swift callback in Objective-C `@try` would still permit an exception to unwind through Swift frames and is not an acceptable solution.
- [Windows UI Automation](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-uiautomationoverview) is a COM-based accessibility framework, not the actual WinUI visual tree. [Electron's debugger API](https://www.electronjs.org/docs/latest/api/debugger) supplies a Chrome DevTools Protocol transport for a renderer. Framework, access mechanism, language, and evidence source are independent choices.

### Relevant constraints and principles

One Recon product can use several implementation languages. An extension may need a host component, target-resident code, or a protocol client; an authoring SDK must not force all three into one binary-plugin model. Native SDKs should expose normal language types and control flow, while shared semantics define what requests, identities, outcomes, and cancellation mean.

Generated implementation plumbing is acceptable. A public SDK whose design is mechanically determined by Rust declarations is not the intended result. Transport details may be hidden; execution location, access prerequisites, and irreversible effects must remain visible.

### Ontology review

**CONCEPT INVENTORY**

- **Host** — the environment running Recon's executable.
- **Target environment** — where the inspected subject exists, such as the host, a simulator, or a connected device.
- **Target** — the selected process incarnation, page, or artifact; its identity comes from its own domain rather than a universal PID or bundle path.
- **Capability** — an inspection or control behavior Recon can offer; it is not a mandatory SDK method or a fixed list of UI operations.
- **Action** — an explicitly exported operation with a stable name scoped to its extension, typed inputs and outcomes, prerequisites, execution location, and effects.
- **Backend** — the implementation of a capability through a particular inspection mechanism.
- **Extension** — the distributable implementation exporting named actions to Recon; it can contain more than one backend or execution component.
- **Language SDK** — the extension-author-facing library, compiler support, and testing facilities for one language.
- **Session** — an access relationship to a particular target, not the extension process, loaded code, or a saved observation.

**CONFLATIONS AND RELATIONSHIPS**

AppKit and browser runtimes describe useful inspection areas, not mutually exclusive application classes. An Electron application can expose native windows, accessibility elements, DOM nodes, and network observations. Several backends may inspect the same target; one capability may be exposed through several actions. The first AppKit action set does not define every action an extension may export. An action is not inherently a UI read, and an action that does not inspect a live target need not invent a target session. A CLI command is a user-facing route to an action, not the action's identity or its Swift method name.

**IDENTITY / LIFECYCLE ISSUES**

Extension/action identity, backend identity, target identity, session identity, native object handles, and immutable observations remain distinct. An exported action's identity is its extension identifier plus its declared action name; renaming a Swift implementation method need not rename that action. A process restart or page replacement cannot be hidden by reusing a display name. Closing access does not imply unloading target-resident code. A handle is meaningful only within its owning backend and session; correlation does not make handles interchangeable.

**ASSESSMENT: COHERENT with these distinctions.** The extension contract does not promote AppKit's current operation list into a universal provider protocol, and it introduces no universal platform enum or UI-node identity. Exporting author-defined actions is distinct from evaluating arbitrary client-supplied code. The unresolved command-exposure and deployment choices below remain explicit draft questions.

### Quality bar

An extension author can implement useful inspection using ordinary language constructs, explicitly export named actions, test them using the language's normal tools, and receive actionable diagnostics for invalid declarations. Adding an action does not require editing a core operation enum or implementing unrelated methods from a fixed provider interface. The author does not write C exports, raw-pointer ownership glue, wire messages, or a second registration manifest. First-party extensions use the same authoring surface; they do not require privileged private APIs to become useful.

The resulting CLI retains source-attributed observations, bounded work, truthful failure and cancellation reporting, and the existing no-focus and privacy guarantees. A demonstration that only serializes an example value is not evidence that the extension architecture works.

### Remaining uncertainty

The Rust CLI/core and purpose-designed Swift SDK direction are established, not an approved macro API or a verified Swift target-resident runtime. Action-to-command exposure, the recommended AppKit execution arrangement, and feasibility evidence remain explicit draft questions. No Swift/arm64e loading or native SDK prototype was exercised while authoring this proposal.

## Existing baseline

- `crates/recon/src/lib.rs` exposes internal modules for integration tests and explicitly disclaims a supported library API. It is not already a reusable core SDK.
- `crates/recon/src/main.rs` parses requests, dispatches to macOS operations, and renders the public output/error contract. `cli/registry.rs` is the existing source of command discovery; exported action metadata must integrate with that registry rather than introduce a competing command catalogue.
- `application.rs` models installed bundles by path, and `inspection/transport.rs` implements authenticated, length-prefixed JSON over a Unix-domain socket. These are useful existing implementations, not universal definitions of an application or extension transport.
- `inspection/` owns session storage, process incarnation checks, bounded messages, and uncertain-close reconciliation. `macos/` owns native access and UI operations. These responsibilities should survive extraction, while Unix transport and macOS identity checks must not be mislabeled platform-independent business logic.
- `helper/ReconServer.m` authenticates requests, maintains access state, and dispatches bounded inspection onto the target's main thread. `ReconCore.m`, `ReconInspect.m`, `ReconFields.m`, and `ReconFacets.m` implement native handles, traversal, fields, and facets. Their user-visible behavior is specified by [proposal.0005](0005-native-macos-inspection.md).
- `loader/recon_loader.c`, `helper/bootstrap.s`, and `tools/build-helper.ts` establish the native artifact and loading boundary. The helper has arm64 and arm64e slices and an exact build identity. These requirements survive a language change.
- The current command surface was exercised with `mise run dev -- --help` during the design discussion. This confirms the existing CLI entry points, not the feasibility of any proposed extension implementation.

Preserve the behavior and safety boundaries, not the incidental allocation of code to the current crate and Objective-C files. `Cargo.toml` currently declares only `crates/recon`; its library exposes internals to tests rather than enforcing the proposed architectural boundaries. There is no existing Swift extension SDK to claim as a working baseline.

## Proposed solution

Make the multi-crate rearchitecture and generic action contract the two connected parts of this change. Retain one Rust CLI, extract shared investigation semantics into `recon-core`, and move macOS host integration into `recon-macos`. Add a purpose-designed Swift SDK that exports explicitly annotated named actions and generates the registration and bridge code beneath ordinary Swift methods.

The recommended first consumer is Recon's existing AppKit native inspector. It must perform real inspection through the SDK and replace the previous traversal implementation at cutover. Existing C/Objective-C runtime support may remain where it supplies the loader, authentication, lifecycle, or exception-containment boundary; extension authors do not write or call its C interface directly.

```text
Host-side Rust crate dependencies

recon ───────────────→ recon-core
  │                       ↑
  └─────────────────→ recon-macos
                          │
                 authenticated native transport
                          │
Target-resident helper    │
  Swift SDK runtime + AppKit extension + native support
```

Rust dependency arrows point toward the imported crate. `recon-macos` depends on `recon-core`, never the reverse; the transport line is a process boundary, not a Cargo dependency. The Swift inspector and its SDK runtime reside in the target's existing helper artifact. No Rust core is injected merely to support Swift. The same action contract must admit implementations that do not use this AppKit execution arrangement.

## Detailed design

The following specifies the recommended draft arrangement. Open questions identify the choices and feasibility evidence that must be resolved before the human approves implementation; this draft is not permission to silently select a different scope.

### Rust workspace rearchitecture

The resulting Rust workspace has exactly these three crates for the first delivery:

| Crate | Owns | Does not own |
| --- | --- | --- |
| `recon` at `crates/recon/` | Executable entry point, argument parsing, the command registry, help/schema presentation, standard-stream delivery, exit behavior, and embedded executable build/privacy metadata | Native framework calls, helper transport, session storage, or the shared action-dispatch implementation |
| `recon-core` at `crates/recon-core/` | Language-neutral action identities and descriptors, typed investigation inputs/outcomes, observation and error contracts, and dispatch/orchestration independent of a platform mechanism | CLI parsing or `clap`, AppKit/Objective-C bindings, bundle-path target identity, macOS permission/signing queries, Unix sockets, or local session files |
| `recon-macos` at `crates/recon-macos/` | macOS application discovery and target resolution, native access and UI operations, privacy/signing queries and responsibility re-execution, continuous no-focus guarding, process-incarnation verification, local native-session persistence, loader integration, and helper transport | Command grammar, a second public error catalogue, or the platform-independent action model |

`recon` depends on `recon-core` and, on macOS, `recon-macos`. `recon-macos` depends on `recon-core`; neither library depends on `recon`. Native framework and macOS-only dependencies move with their implementations instead of remaining attached to the CLI crate. The core must compile and be testable without the CLI, a Mac, or a Swift runtime.

This is extraction and migration of existing responsibilities, not three new facades around the old library. Move the platform-independent portions of the current observation/request/error models into the core, leaving CLI-specific parsing and schema presentation in `recon`. Move the current `macos/` implementation and the macOS-specific identity, store, lifecycle, and transport responsibilities in `inspection/` into `recon-macos`. Split mixed modules such as execution-context reporting at the boundary between platform facts and CLI rendering rather than moving them wholesale into a supposedly portable core.

Migrate every import and caller to the owning crate. Shared behavioral tests belong with the core; macOS integration tests belong with the native integration; executable contract tests remain with `recon` and exercise the assembled product. Do not retain forwarding modules, dead re-exports, duplicate dispatch paths, or a compatibility library in `recon` that preserves the former ownership. Keep the existing unsupported-host behavior where a macOS capability is unavailable.

Register all three crates in the workspace with the shared version and lint policy. `mise run check`, rustdoc, and all-target builds must cover every member, and Linux verification must exercise the portable core without linking native frameworks. Update native build/test tasks, executable build metadata, and packaging references where extraction changes their source paths. The installed executable and matched native artifact names remain unchanged.

Existing Rust-backed macOS capabilities continue to work without being rewritten into Swift. The AppKit native-read path becomes the first foreign-language implementation behind the action boundary. The first delivery explicitly assembles one implementation per exported action; no runtime provider-priority or fallback algorithm is introduced. A missing native action implementation is an inspection failure, not permission to substitute an accessibility tree.

### Language-neutral contract

The contract describes semantics rather than prescribing a Rust type layout. An extension exports a set of named actions; the core dispatches by extension identity and action name, not by a universal enum of operations or a fixed provider protocol. Each action describes its typed input/result schema, execution location, target/session requirements where applicable, prerequisites, bounds, and declared effects. Each assembled implementation also has a build identity. Only explicitly exported declarations are callable; ordinary public methods are not automatically actions.

An action author may choose a new name and supported typed signature without modifying a predefined inspection interface. This does not imply that every language type can cross the boundary or that an unbounded operation is acceptable. Generated adapters validate supported inputs and outcomes, and the runtime enforces the action's access and execution contract. Generic registration must not reduce all author-facing inputs/results to untyped dictionaries.

The first AppKit consumer exports the existing `windows`, `tree`, `node`, `find`, and `fonts` reads. These are concrete actions, not required members of every extension. Their bounds, fields, handles, error classifications, and observable outcomes remain those of proposal.0005. The existing open/reconcile/close access lifecycle stays runtime-owned rather than becoming author boilerplate. Discovery of an action does not claim it is currently usable: access may still be unavailable for the selected target.

All CLI routes continue to use one command registry for parsing, help, schemas, prerequisites, effects, and published errors. Generated extension metadata is not a separately maintained command catalogue. The draft leaves open whether newly exported actions receive explicit command mappings, generated registry entries, or a registry-defined generic invocation command; that choice must be resolved before implementation approval. Do not silently limit exportable actions to the five current AppKit reads to avoid making it.

For an action requiring a target session, the host routes the request only to the backend/session to which its identity belongs. Backend-local handles are opaque to the core. Display names, bundle identifiers, numeric equality with another backend's handle, and correlation results are not interchangeable authority. Existing AppKit session descriptors and public handle spellings remain compatible; this proposal does not rewrite them merely to advertise an internal backend identifier.

Wire encoding is private bridge machinery, distinct from the author-facing API. Preserve the current AppKit protocol version, framing, and operation spellings for its existing reads. Adapters connect those operations to generated action dispatch; the core action model must not inherit the native transport's fixed operation enum. The compiled action set remains closed to undeclared names, runtime-loaded code, selectors, and expressions supplied by a client. Extensible source declarations are not an arbitrary evaluation endpoint, and do not require embedding a Rust runtime in every target.

### Swift authoring SDK

Provide a Swift package under `sdks/swift/` with one author-facing `ReconExtension` library and a macro implementation target. Keep bridge and native-support internals separate from the public authoring module. The AppKit implementation lives under `extensions/appkit/` and depends on the SDK as a normal Swift package dependency.

The public API uses Swift structs and enums, ordinary methods with typed arguments/results, normal error handling, and explicit actor isolation. Method and type names follow Swift conventions, not transliterated Rust naming. Native objects remain local to the implementation and are never serialized as pointers or foreign handles.

Use an enclosing extension declaration and explicit per-method action annotations to generate registration. The draft spelling is `@ReconExtension(id: "appkit")` for the enclosing implementation and `@ReconAction("windows")` for an exported method. The exported name is independent of the Swift method name. The name and shape are not tied to native UI inspection: another declaration may export a different named action without conforming to a five-method protocol.

For example, the AppKit extension can export a method taking `NativeWindowsRequest` and returning `NativeWindowsObservation` as its `windows` action, and a method taking `NativeTreeRequest` and returning `NativeTreeObservation` as `tree`. These proposed SDK types express AppKit-specific requests and observations, not a mandatory shape for every action. The SDK supplies the current native operations' field, bound, incompleteness, and failure semantics; authors do not recreate wire schemas. A test extension can declare its own supported input/result value types and an unrelated action name using the same mechanism.

Generated descriptors and invocation adapters derive from the annotated method signatures and explicit metadata. The author declares the action's effects, prerequisites, and execution requirements where they cannot be derived from types; the macro must not infer safety from a method name or body. Generated registration is the runtime's source of exported actions, not a second handwritten manifest or a hardcoded action list. Assembly rejects duplicate extension identifiers and duplicate action names within an extension rather than silently selecting an implementation. The same local action name in two distinct extension namespaces is not a collision.

Macros operate on source syntax and generated Swift must type-check. The SDK must define supported method and value-type forms, diagnose unsupported declarations or encodings at the authoring site, and reject native objects, raw pointers, or closures as cross-boundary values. Macro registration must not promise arbitrary Swift reflection or automatic schema derivation for every possible Swift type. Pin the concrete annotation metadata and supported signature forms through the feasibility proof before approval rather than presenting these illustrative spellings as a finished API.

The AppKit methods execute synchronously on `@MainActor` inside the target-resident helper, because they inspect live native state within a bounded traversal. Other actions can have different isolation and use `async throws` when suspension is required; there is no universal main-actor provider protocol or mandatory asynchronous wrapper around every getter. Generated adapters preserve the declaration's isolation and error behavior rather than detaching work to bypass it.

The SDK supplies an execution context with the applicable access scope, deadline, and cancellation observation. AppKit actions additionally use typed native-handle facilities scoped to their backend/session; an unrelated action does not need a native handle registry. Authentication secrets remain runtime-owned. The runtime owns queueing and checks access revocation/deadline before invoking an action. Traversal loops observe cancellation and deadline through the context; they cannot safely interrupt a getter already executing.

The API must distinguish normal Swift errors from target field-read failures and access/lifecycle failures. Missing information, unavailable fields, truncated traversal, and stale objects retain the current explicit outcome distinctions. Native values, such as geometry, are translated into the existing observation shapes inside the AppKit actions; the core does not retain live `NSView` or `NSWindow` objects.

Macros generate Swift, not Objective-C source. Exception-prone native access is implemented through typed SDK native-access primitives whose actual getter invocation and Objective-C catch occur entirely in Objective-C before returning to Swift. Supported getter signatures and fields are explicitly allowlisted. A client cannot supply a selector, key path, expression, or pointer for arbitrary execution. Adding an accessor family is SDK implementation work, not a requirement that extension authors hand-wrap a C API. The SDK must not promise to catch an arbitrary Objective-C exception thrown through arbitrary Swift code.

### Execution, lifecycle, and failure semantics

Keep process boundaries where the inspection mechanism requires them. The recommended AppKit arrangement loads the Swift backend and SDK runtime through the existing `recon_helper_main` entry point, C loader, and assembly bootstrap. Source-linked native support may retain existing authentication and lifecycle code. The Swift traversal is not a host wrapper that continues delegating every observation to the previous Objective-C walker.

The host remains responsible for the existing privacy-identity and continuous no-focus guarded path. No extension load or dispatch bypasses the guard or changes the meaning of `recon context`. Metadata discovery and command help do not inject code, request permission, or instantiate a target-side backend. The existing explicit session-opening modes remain the entry point to target-side access.

The SDK/runtime validates access and bounds before dispatch and rechecks revocation and deadline before queued work starts. Closing a session invalidates its access and native handles but does not unload the helper or free objects still used by an in-flight read. A queued operation that loses access must not begin; a started operation retains the existing bounded execution and reconciliation rules.

Cancellation is cooperative, not rollback. A deadline or cancellation cannot safely interrupt a blocked native getter. Preserve the current distinction between work that never started and target execution that may continue, including the published `targetExecutionMayContinue` indication where applicable. Do not automatically retry, replace the helper, restart the target, or dispatch a second read while unresolved target execution prohibits it.

Keep traversal and field collection next to native objects. Return bounded observations in batches through the existing transport rather than making one Rust/Swift or IPC round trip per node/property. Do not introduce avoidable copies of the native graph into a universal intermediate object model.

Known failures map to the current typed native-inspection classifications. Unknown foreign errors become a bounded `INSPECTION_FAILED` result, not a new public error code or an empty successful observation. Protocol mismatches remain `INSPECTION_PROTOCOL_MISMATCH`; stale sessions and stale nodes remain distinct. A crash, trap, invalid memory access, or transport loss is not advertised as a recoverable Swift error. The host must report the observed failure without fabricating successful cancellation or complete evidence.

### Assembly, compatibility, and trust

The first delivery assembles first-party extensions from explicit build inputs and releases them with Recon. There is no plugin-directory scan, runtime package installation, arbitrary library path, independent binary SDK compatibility promise, or extension marketplace. SDK compatibility and native helper protocol compatibility are separate concerns. Source APIs may evolve through reviewed changes; incompatible helper builds continue to fail explicitly.

The production artifact remains the matched CLI, loader, universal helper, and helper build-ID set. Fold the AppKit Swift implementation and required SDK/native support into the helper artifact rather than making users manually install an extension runtime. Include behavior-affecting Swift sources, native support, resolved package inputs, and build configuration in helper build identity; do not use timestamps or checkout paths as identity. Preserve the existing exact-match and target-restart requirements on helper replacement.

Extensions are trusted code, not sandboxed by macros or a transport protocol. They must not receive broader target access merely because they use an SDK. The initial target helper retains its closed, authenticated operations and existing research/cooperative access distinctions. This proposal grants no additional authority to weaken host security, modify targets, or run protected-target experiments.

### First consumer, cutover, and evidence

The recommended first delivery includes a real Swift AppKit native backend covering all existing native reads and facets, the SDK it uses, and the core/host separation needed to dispatch it. Keeping the old walker as the actual implementation while adding an unused Swift facade does not satisfy the proposal. Remove the displaced traversal/field/facet implementation at cutover; retain native support only where it continues to implement a necessary runtime or safe accessor boundary, not as a fallback backend.

Before broad migration, establish the pinned Swift toolchain and exercise a small helper through the existing loader on arm64 and arm64e. The proof must include a target-main-thread AppKit read, allocation/ownership across the generated adapter, safe native getter-exception containment, queued cancellation/revocation, and failure containment without unwinding a foreign exception through Swift. Build, signing, Swift runtime dependencies, and helper build identity must be reproducible. A failed proof returns the draft design to review; it is not permission to omit arm64e or deliver only cooperative access. The timing and authority for this proof are an explicit Open question.

Behavioral verification derives from proposal.0005 and this section. Existing fixture coverage must run against the Swift implementation and demonstrate native fields/facets, bounds and incompleteness, weak-handle reuse/reparenting/staleness, process replacement rejection, queued-close races, timeout behavior, protocol/build mismatch rejection, and the no-focus contract. SDK tests must register and invoke a test-only action with a non-AppKit name and its own supported input/result value types through the generated dispatcher, without adding a core operation case or a required SDK method. Verify its typed outcome and error behavior, reject undeclared actions and duplicate names in one extension, and permit the same local name in different extension namespaces. Cover unsupported signatures and meaningful author diagnostics rather than snapshotting generated source text.

Use the ordinary Swift test tools for SDK/backend tests and the existing Rust/native integration tasks for end-to-end evidence. Add enforceable mise tasks for the new toolchain and checks. The repository gate remains `mise run check`; macOS-specific Swift and live inspection checks require documented macOS tasks/CI rather than pretending to run on the Linux gate. Required live evidence includes the currently supported cooperative and research paths; new research-target authorization must be obtained where needed.

Write the extension-author guide from this approved design before the implementation. It must explain named-action declarations, generated registration, typed values, command exposure, execution location, native exception limitations, cancellation, local testing, and packaging with a real AppKit example. Do not describe the Swift SDK as available before it exists.

## Compatibility

The existing CLI grammar, help/schema source, success/error envelopes, observation field semantics, session modes, native handle semantics, no-focus behavior, and privacy identity remain unchanged for current commands. Existing AppKit actions back those same commands; moving code into crates or adding annotations does not rename the public surface. Stored native session descriptors keep their existing format. The exposure of additional named actions remains an explicit design question, not a promise of an unspecified new command family.

An updated helper build still requires a matching CLI and, when an older helper remains resident, the existing explicit target-restart workflow. There is no hot replacement, protocol downgrade, or old-backend fallback. Moving code to a Swift SDK does not expand supported host versions, target architectures, target access, or inspection capabilities.

Issue #74 is related work, not a prerequisite or authority for a simultaneous Rust-helper port. The recommendation is to defer its broad rewrite while reviewing this architecture. This draft does not close the issue, approve a replacement implementation, or alter historical proposal.0005. If this direction is accepted, update #74 to identify which remaining native runtime work is still relevant rather than maintaining competing production walkers.

## Implications on adoption

Recon users continue to install one product through the existing Homebrew path and use the same commands and grants. The build must package required runtime support; users must not install SwiftPM or an extension development environment to inspect an application from a bottle.

Contributors and source builds gain three explicit Rust crate boundaries, a pinned Swift toolchain, Swift package dependency resolution, SDK tests, and macOS build verification. The exact version floor and arm64e/runtime packaging evidence remain approval prerequisites. The Swift SDK is initially consumed from the same source revision, not published as an independently compatible package.

A normal release rollback can reinstall the previous matched artifact set; it cannot safely unload a newer helper from an already-running target. Target restart and privacy re-granting retain their existing meanings. Approval of this proposal does not itself authorize a package publication, release, or tap update.

## Scope

- The complete Rust workspace migration into `recon`, `recon-core`, and `recon-macos`, including dependency direction, existing callers, test ownership, and build/packaging references.
- A purpose-designed Swift extension SDK for explicitly declared named actions, including macro-generated registration/adapters, necessary runtime support, native-access safety primitives, and a real AppKit consumer using that public authoring surface.
- Clean migration of the existing native observation path, with reproducible matched-artifact builds and the current supported target/access matrix preserved.
- Swift SDK author documentation and behavioral/native verification of both generic action authoring and the migrated AppKit path.

This is one integration change with one real consumer, not a commitment to implement every future backend in a single pull request. Crate extraction alone, an unused SDK, or a fixed AppKit provider protocol with annotations added on top does not satisfy the proposed architecture.

### Out of scope

- Designing or shipping additional language SDKs or new platform inspectors; those remain future work rather than approval prerequisites for this change.
- Replacing the Rust CLI with a native-language executable, or making a consumer-facing embedding API for unrelated applications.
- A public binary plugin ABI, dynamic third-party package loading, automatic extension discovery, untrusted-code sandboxing, or a marketplace.
- A universal UI tree, generic object evaluator, arbitrary selector invocation, new network capture, persistent evidence storage, streaming, or new user-facing inspection capabilities.
- Rewriting the C loader or assembly bootstrap, implementing #74 alongside this proposal, or deleting native runtime support merely to achieve language purity.
- Changing host security settings, signing requirements, permission ownership, or the no-focus policy to accommodate a new runtime.

## Preview

- Repository shape: a three-crate Rust CLI workspace plus a Swift SDK and native helper; the proposal itself is rendered by the existing documentation site.
- Implementation artifact: a locally installable development CLI with its matched loader/helper/build-ID set, plus the source-consumable Swift package and AppKit extension from the same checkout. This is the cheapest realistic preview because a standalone SDK sample would not exercise target-side access, packaging, or the existing command contract.
- Exercise: use the fixture's existing native session and view commands through the preview executable, inspect concrete fields and stale-handle behavior, then modify a supported Swift action implementation and rerun its native-language tests and the fixture path. Exercise the SDK test extension's independently named action through generated dispatch to show that action authoring is not limited to AppKit's operation list. The readiness report supplies exact paths and commands that were actually exercised.
- Private access: use a local build prefix and local documentation preview. Do not publish prerelease SDK packages, make a public binary release, or install an unreviewed tap formula merely to obtain a preview. The requested draft PR is for review of this proposal.
- Completion cleanup: remove only preview-owned installations and fixture instances after review, retain source and verification evidence, and leave unrelated apps, grants, sessions, and development installations untouched. Helper residency still requires the target's normal restart lifecycle; cleanup does not authorize terminating unrelated targets.

## Policies and decisions checked

- [VISION.md](../VISION.md) — retain the Rust CLI, capability-oriented integration, distinct evidence sources, explicit effects, and separately proposed target environments. Acceptance would update the vision to describe the SDK architecture and only the capabilities actually delivered; this draft does not rewrite the vision.
- [policy.0001](../policies/0001-cli-command-and-error-contract.md) — one command registry remains authoritative for parsing, help, schemas, effects, and published errors. Action export and CLI command exposure are distinct; explicit mappings, generated registry entries, or a registry-defined invocation command must not create a second grammar. The exposure choice is unresolved below.
- [policy.0003](../policies/0003-ui-no-focus.md) — all `ui` requests and dispatched target work retain the guarded path and interference reporting. Thread dispatch or an SDK annotation is not evidence of no-focus behavior. Existing setup exceptions are unchanged.
- [decision.0001](../decisions/0001-macos-privacy-identity.md) and [decision.0002](../decisions/0002-macos-context-privacy-identity.md) — read as superseded history; their identity/reporting rationale survives through decision.0005, not through reinstating their former mandatory Developer ID distribution requirement.
- [decision.0003](../decisions/0003-recon-owned-native-loader.md) — retain the C loader, bootstrap entry point, kernel-verified process binding, authenticated bounded helper protocol, explicit research access, and no hot replacement. Compiled, explicitly exported actions remain a closed set for the assembled build; accepting arbitrary client-supplied code, selectors, or runtime-loaded implementations would conflict and is excluded.
- [decision.0004](../decisions/0004-homebrew-bottle-releases.md) — continue shipping the matched native artifact set through the existing formula/bottle path. No new visible executable or separate public runtime download is proposed. Validate source and bottle packaging with Swift before claiming adoption is unchanged.
- [decision.0005](../decisions/0005-homebrew-ad-hoc-privacy-identity.md) — keep `me.malstrom.recon`, executable-owned grants, re-execution, preflight-only context reporting, and the ad-hoc Homebrew signing arrangement. A separate host-side extension executable could change permission attribution; that consequence is not assumed away and is part of the deployment choice below.

No policy or decision is superseded by this draft. If feasibility requires violating one of these constraints, return the conflict to the human rather than treating an SDK implementation detail as authority to change it.

## Future directions

- A C# SDK could use a purpose-designed .NET authoring experience, with attributes and [source generators/analyzers](https://learn.microsoft.com/en-us/dotnet/csharp/roslyn-sdk/) where useful. A Windows inspector would need its own COM, dispatcher, access, and evidence contracts; Windows UI Automation and native WinUI inspection remain distinct. No interface signatures, package design, cross-language API review, or executable deliverable are decided or required here.
- A Kotlin SDK could use idiomatic Kotlin and [KSP](https://kotlinlang.org/docs/ksp-overview.html) where useful for Android-side action declarations. Its interface, lifecycle mapping, packaging, and target-access design require separate grounding and a later proposal. Host-side Android tooling written in Rust need not adopt Kotlin simply because the target is Android.
- UIKit, browser, and Electron backends could reuse capability composition without inheriting AppKit handles or assuming a one-process application. A TypeScript SDK could be appropriate for runtime-local JavaScript integration without requiring every browser protocol client to use JavaScript.
- Independent SDK releases, third-party extension installation, and additional hosting arrangements could be proposed once real consumers establish compatibility and trust requirements. They are not implied by source-level first-party extensibility.

## Alternatives considered

- **Rust-only implementation, including #74.** Offers one language for most logic and potentially shared protocol/lifecycle code. It does not provide the requested native-language authoring experience and retains native ABI/toolchain constraints. Individual Rust backends remain valid; Rust-only is not the proposed architectural policy.
- **Native-language CLIs embedding a Rust library.** Allows a native executable to own its runtime directly, but couples provider language to product assembly and adds embedding APIs without an established consumer. Native providers do not require replacing the current CLI.
- **Generate the public SDK from Rust declarations.** Reduces bridge maintenance, but makes Rust representations authoritative over Swift ergonomics. Generated adapters beneath a hand-designed authoring API remain an implementation option; generated API shape is not the product design.
- **Independent executable extensions for everything.** Gives a convenient language-neutral protocol and some host-side failure isolation, but does not remove target-resident inspection, adds process/lifecycle and permission-attribution concerns, and cannot isolate a target from code already injected into it. Use process boundaries where the mechanism needs them rather than mandate one deployment form.
- **A fixed native-inspection provider protocol.** Makes the first five AppKit reads easy to enumerate, but confuses one consumer's operations with the extension contract. Every new kind of action would require another central interface change. Renaming the protocol or generating its conformance would not remove that restriction.
- **Macro-declared named actions over shared semantics, with explicit first-party assembly.** Recommended. Keeps authoring open to new action names and typed values without opening a client-side evaluator or a public binary plugin ecosystem. It preserves a coherent CLI contract while separating action declaration from the unresolved command-exposure choice.

## Open questions

- [NEEDS CLARIFICATION: Choose how exported named actions become user-callable: explicit mappings to commands in the existing registry, generated command entries composed into that same registry, or a registry-defined generic invocation command. The SDK must not limit exportable actions to the existing AppKit reads, and help/schema/effect/error discovery must remain authoritative. Settle command naming and schema representation before approval.]
- [NEEDS CLARIFICATION: Confirm the recommended AppKit execution arrangement: a Swift backend inside the existing target-resident helper, retaining narrow C/Objective-C loader/runtime and exception-safe access support. A host-side Swift extension would be a different design and must establish privacy attribution and explain which native functionality it owns; it is not an equivalent substitute for a Swift AppKit inspector.]
- [NEEDS CLARIFICATION: Establish the supported pinned Swift toolchain and authorize the bounded feasibility proof before implementation approval, or explicitly authorize it as an initial implementation gate. Record arm64/arm64e loading, runtime packaging, safe native exception handling, and macro declaration/signature/schema evidence before treating the SDK/runtime design as settled. No such proof or additional research-target authorization exists in this draft.]
