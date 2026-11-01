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

Restructure Recon into focused Rust crates for shared execution, application discovery, WindowServer operations, accessibility, native access, privacy identity, and no-focus guarding, rather than moving the current monolith into one macOS crate. Introduce a Swift-native extension SDK inspired by Neon: export ordinary functions that accept supported values, return Recon-compatible types, and use normal Swift error handling. Prove the architecture with an independently owned AppKit inspector while preserving existing command, access, identity, and observation contracts.

## Motivation

Recon's owner wants inspection capabilities to be implemented in the language that best fits the inspected technology, starting with Swift for AppKit. Extension authors should expose functions that do useful work and return Recon-compatible values, not implement a provider interface or manage a Rust ABI. Macros should remove export and conversion plumbing without defining which kinds of inspection may exist.

The current Rust executable, Objective-C helper, and C loader already cross language and process boundaries, but those boundaries are private implementation details. There is no supported extension-authoring experience. Inside the Rust crate, application discovery, WindowServer capture, accessibility, native access, privacy, and UI guarding also share one platform module tree. Naming that entire collection `recon-macos` would preserve the coupling rather than establish focused ownership.

Issue [#74](https://github.com/markmals/recon/issues/74) proposes porting the native helper to Rust. Language uniformity could simplify parts of maintenance, but does not answer how different inspection technologies should integrate. Conversely, changing the executable's language would not by itself produce a reusable extension contract. The problem is the missing separation between CLI, shared investigation logic, platform integration, and extension authoring, not the language of `main`.

## Domain grounding

### Established context

- [Node-API](https://nodejs.org/api/n-api.html) is a deliberately designed native-addon API with binary compatibility guarantees across Node versions. Its separation of an authoring interface from implementation details is relevant; its independent binary-compatibility commitment is not a prerequisite for Recon.
- [Neon's function model](https://neon-rs.dev/docs/functions/) exports Rust functions that interact with a JavaScript host through a context and return JavaScript-compatible values or errors. Its [export macro](https://docs.neon-bindings.com/neon/attr.export) also supports ordinary Rust argument/result types through `TryFromJs` and `TryIntoJs` conversions. The relevant authoring model is functions plus supported host values and conversions, not a domain-specific provider interface.
- [Neon's value types](https://docs.neon-bindings.com/neon/types/) model JavaScript values; its [contexts and handles](https://docs.neon-bindings.com/neon/context/) manage access to a garbage-collected runtime. Recon is not a JavaScript engine. This proposal takes the export/conversion ergonomics as precedent, not V8-style object handles, JavaScript inheritance, Node's worker pool, or an in-process Node addon ABI.
- [UniFFI](https://mozilla.github.io/uniffi-rs/latest/) generates foreign-language bindings to Rust libraries. Binding generation can implement a bridge, but does not establish an idiomatic extension API or its lifecycle semantics.
- Swift's [attached macros](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0389-attached-macros.md) can generate wrapper functions, peer declarations, and members from annotated declarations. [Swift Package Manager supports macro targets](https://www.swift.org/blog/swift-5.9-released/#swift-package-manager). This supports explicit function exports without requiring an SDK protocol for every possible action. Macro expansion works from syntax; generated code must still type-check, preserve actor isolation, and use supported value encodings. Macros do not inherently provide runtime reflection, another process's main thread, or a security boundary.
- Apple's [thread-safety guidance](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html) constrains AppKit view access. The existing inspector runs in the target process and dispatches native reads to that application's main thread. A Swift host executable or host-side `@MainActor` annotation cannot replace target-side execution.
- Apple explicitly states that [Objective-C exceptions must be caught before reaching Swift code](https://developer.apple.com/documentation/swift/handling-cocoa-errors-in-swift). Swift `throws` is not an Objective-C exception boundary. Wrapping a Swift callback in Objective-C `@try` would still permit an exception to unwind through Swift frames and is not an acceptable solution.
- [Windows UI Automation](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-uiautomationoverview) is a COM-based accessibility framework, not the actual WinUI visual tree. [Electron's debugger API](https://www.electronjs.org/docs/latest/api/debugger) supplies a Chrome DevTools Protocol transport for a renderer. Framework, access mechanism, language, and evidence source are independent choices.

### Relevant constraints and principles

One Recon product can use several implementation languages and several inspection mechanisms on the same operating system. An extension may need host code, target-resident code, or a protocol client; an SDK must not force all three into one binary-plugin model. Crate boundaries follow coherent responsibilities and dependency direction, not the operating system alone. A Rust crate, a Swift package, an extension, and a deployed process are different boundaries and need not correspond one-to-one.

The authoring boundary is a function with supported inputs and a Recon-compatible result. The Swift SDK owns the conversion rules and runtime-facing value types; authors use Swift functions, values, `throws`, and actor isolation rather than translating Rust ownership conventions. Generated implementation plumbing is acceptable. A public SDK mechanically determined by Rust declarations, a fixed AppKit method list, or arbitrary `Codable` serialization is not the intended result. Transport may be hidden; execution location, access prerequisites, and irreversible effects must remain visible.

### Ontology review

**CONCEPT INVENTORY**

- **Host** — the environment running Recon's executable.
- **Target environment** — where the inspected subject exists, such as the host, a simulator, or a connected device.
- **Target** — the selected process incarnation, page, or artifact; its identity comes from its own domain rather than a universal PID or bundle path.
- **Capability** — an inspection or control behavior Recon can offer; it is not a mandatory SDK method or a fixed list of UI operations.
- **Action** — an explicitly exported function with a stable name scoped to its extension, supported inputs and Recon-compatible outcomes, prerequisites, execution location, and effects.
- **Backend** — the implementation of a capability through a particular inspection mechanism.
- **Extension** — the distributable implementation exporting named actions to Recon; it can contain more than one backend or execution component.
- **Language SDK** — the extension-author-facing library, compiler support, and testing facilities for one language.
- **Recon value** — bounded data accepted by the shared execution/observation contract, constructed directly with SDK types or through an explicit supported conversion; not a live native object or an arbitrary serialized Swift object.
- **Session** — an access relationship to a particular target, not the extension process, loaded code, or a saved observation.

**CONFLATIONS AND RELATIONSHIPS**

AppKit and browser runtimes describe useful inspection areas, not mutually exclusive application classes. An Electron application can expose native windows, accessibility elements, DOM nodes, and network observations. Several backends may inspect the same target; one capability may be exposed through several functions. A crate groups one coherent implementation responsibility, not everything available on an OS. The AppKit extension owns NSView traversal independently of WindowServer and accessibility. An action is not inherently a UI read, and an action that does not inspect a live target need not invent a target session. A CLI command is a user-facing route to an action, not the action's identity or its Swift function name.

**IDENTITY / LIFECYCLE ISSUES**

Extension/action identity, backend identity, target identity, session identity, native object handles, and immutable observations remain distinct. An exported action's identity is its extension identifier plus its declared export name; renaming the Swift implementation function need not rename that export. A process restart or page replacement cannot be hidden by reusing a display name. Closing access does not imply unloading target-resident code. A handle is meaningful only within its owning backend and session; correlation does not make handles interchangeable.

**ASSESSMENT: COHERENT with these distinctions.** The extension contract is functions over Recon-compatible values, not a universal provider protocol or a platform-wide backend. SDK conversion is distinct from native-object identity and arbitrary object serialization; a common value model is not a universal UI tree. The proposed mechanism-level crate partition, command exposure, and deployment choices remain subject to human review.

### Quality bar

An extension author can export an ordinary function, accept supported Swift/Recon types, return a Recon-compatible value or throw, and test the function using normal Swift tools. They do not implement unrelated methods, edit a core operation enum, construct wire messages, or maintain a second schema/registration manifest. New names do not require new SDK entry points; new value types require an explicit supported conversion rather than accidental serialization. First-party extensions use the same surface. The generic SDK must be usable without importing AppKit or adopting its domain types.

The resulting CLI retains source-attributed observations, bounded work, truthful failure and cancellation reporting, and the existing no-focus and privacy guarantees. A demonstration that only serializes an example value is not evidence that the extension architecture works.

### Remaining uncertainty

The Rust CLI/core and Neon-inspired Swift authoring direction are established, not an approved concrete conversion API or a verified target-resident runtime. The focused crate partition below is a draft design, not a permanent crate count. Crate ownership, action-to-command exposure, the recommended AppKit execution arrangement, and feasibility evidence remain explicit review questions. No Swift/arm64e loading or native SDK prototype was exercised while authoring this proposal.

## Existing baseline

- `crates/recon/src/lib.rs` exposes internal modules for integration tests and explicitly disclaims a supported library API. It is not already a reusable core SDK.
- `crates/recon/src/main.rs` parses requests, dispatches to macOS operations, and renders the public output/error contract. `cli/registry.rs` is the existing source of command discovery; exported action metadata must integrate with that registry rather than introduce a competing command catalogue.
- `application.rs` models installed bundles by path, and `inspection/transport.rs` implements authenticated, length-prefixed JSON over a Unix-domain socket. These are useful existing implementations, not universal definitions of an application or extension transport.
- `inspection/` owns session storage, process incarnation checks, bounded messages, and uncertain-close reconciliation. `macos/` owns native access and UI operations. These responsibilities should survive extraction, while Unix transport and macOS identity checks must not be mislabeled platform-independent business logic.
- `macos/ui/` currently combines WindowServer inventory/capture, AX reads and tree traversal, native-access orchestration, signing/permissions/responsibility re-execution, no-focus monitoring, and AX/native correlation. `macos/workspace.rs` and `metadata.rs` implement application discovery and bundle metadata separately. These existing responsibilities provide concrete extraction candidates; the common `macos` parent is not evidence that they belong in one library.
- `helper/ReconServer.m` authenticates requests, maintains access state, and dispatches bounded inspection onto the target's main thread. `ReconCore.m`, `ReconInspect.m`, `ReconFields.m`, and `ReconFacets.m` implement native handles, traversal, fields, and facets. Their user-visible behavior is specified by [proposal.0005](0005-native-macos-inspection.md).
- `loader/recon_loader.c`, `helper/bootstrap.s`, and `tools/build-helper.ts` establish the native artifact and loading boundary. The helper has arm64 and arm64e slices and an exact build identity. These requirements survive a language change.
- The current command surface was exercised with `mise run dev -- --help` during the design discussion. This confirms the existing CLI entry points, not the feasibility of any proposed extension implementation.

Preserve the behavior and safety boundaries, not the incidental allocation of code to the current crate and Objective-C files. `Cargo.toml` currently declares only `crates/recon`; its library exposes internals to tests rather than enforcing the proposed architectural boundaries. There is no existing Swift extension SDK to claim as a working baseline.

## Proposed solution

Make responsibility-level decomposition and the function/value extension boundary the two connected parts of this change. Retain one Rust CLI and a portable `recon-core`; extract the existing mechanisms and safety services into focused crates instead of creating an umbrella `recon-macos`. Add a Swift-native SDK modeled on Neon's author experience: ordinary exported functions, host-compatible values and conversions, optional execution-context access, and generated export plumbing.

The recommended first consumer is Recon's existing AppKit native inspector. It must perform real inspection through the SDK and replace the previous traversal implementation at cutover. Existing C/Objective-C runtime support may remain where it supplies the loader, authentication, lifecycle, or exception-containment boundary; extension authors do not write or call its C interface directly.

```text
Host-side product assembly

recon
  ├─ recon-core
  ├─ recon-applications
  ├─ recon-window-server
  ├─ recon-accessibility
  ├─ recon-native-access ── authenticated transport ── target helper
  ├─ recon-macos-privacy
  └─ recon-ui-guard

Target helper: Swift SDK runtime + AppKit extension + native support
```

The tree shows the CLI's assembled dependencies, not a requirement that every capability depend on every other one. Mechanism/support crates use the portable core contracts and only the focused services they actually require; the core imports none of them. The transport line is a process boundary, not a Cargo dependency. The AppKit Swift package remains separate from the Rust native-access crate. No Rust core is injected merely to support Swift.

## Detailed design

The following specifies the recommended draft arrangement. Open questions identify the choices and feasibility evidence that must be resolved before the human approves implementation; this draft is not permission to silently select a different scope.

### Rust workspace rearchitecture

Extract the current implementation along the following focused ownership boundaries. These are concrete draft crates for behavior that already exists, not an exhaustive taxonomy or a fixed limit on future crates:

| Crate | Responsibility |
| --- | --- |
| `recon` | Executable entry point, argument parsing, one command registry, help/schema presentation, product assembly, standard-stream delivery, exit behavior, and embedded executable build/privacy metadata |
| `recon-core` | Portable function/action identities, export descriptors, supported value/conversion semantics, shared observation/error primitives, and invocation orchestration; no platform I/O or capability-specific model catalogue |
| `recon-applications` | Application-bundle metadata, indexed/running application discovery, target resolution, and existing launch behavior; macOS is its current implementation, not a requirement that every inspector import it |
| `recon-window-server` | The existing macOS WindowServer/SkyLight window inventory and capture mechanisms, including their private native calls; no AX tree or NSView traversal |
| `recon-accessibility` | AX element access, attributes, window-element resolution, menu handling, and the existing accessibility-tree traversal/model |
| `recon-native-access` | Process-incarnation-bound helper access: loader integration, artifact/build matching, research-access prerequisites, authenticated sessions, local persistence, bounded transport, and open/reconcile/close lifecycle; no NSView walker |
| `recon-macos-privacy` | Executable-owned privacy identity, signing/permission facts and requests, responsibility re-execution, and the child-output/signal behavior that makes that boundary truthful |
| `recon-ui-guard` | The continuous no-focus monitor, scope/effect attribution, and interference classification shared by UI mechanisms; not another window/accessibility inspector |

Each crate lives under `crates/<crate-name>/`. `recon` assembles the product and depends on the capability/support crates only on supported hosts. Those crates depend inward on `recon-core`, never on the executable. A capability may use a focused service where it needs one, but WindowServer, accessibility, and AppKit observation do not require one another merely because they run on macOS. Keep the dependency graph acyclic. Low-level monitoring primitives used by the guard must not invoke a guarded inspector and recurse through product dispatch.

The generic core stays small: portable invocation/value/error semantics belong there, not every platform-independent request struct. Capability-specific request/result types and pure traversal logic move with their capability. The Swift AppKit implementation lives in `extensions/appkit/`, with its native-access support modules separated from the generic SDK. It is not a Rust `recon-appkit` placeholder or part of the WindowServer/AX crates. The winctl-inspired WindowServer operations, AX observations, and NSView observations remain distinct evidence sources; this extraction adds no new UI input.

This is migration of implementations, not new facades around the old library. Split the current `macos/ui/` modules among their owners; move the relevant `application/` and `ui/` models with their capabilities; move `inspection/`, `macos/inject/`, `inspection_access.rs`, and native-session/artifact integration into native access. Keep CLI parsing/rendering in `recon`. Context reporting composes facts from the owning crates rather than becoming an OS-wide implementation library. AX/native correlation belongs in product composition over the two sources; it must not make either inspector own the other.

Migrate every import and caller to its owner. Behavioral tests move with the responsibility they specify; executable contract and multi-mechanism integration tests exercise the assembled product. Do not retain an umbrella platform crate, forwarding modules, dead re-exports, duplicate dispatch paths, or a compatibility library preserving the former ownership. Preserve unsupported-host reporting, executable identity, native artifact names, privacy grants, and continuous guarded dispatch throughout extraction. Splitting crates must not create a command path around the guard or the privacy boundary.

Register every real crate in the workspace with the shared version and lint policy. `mise run check`, rustdoc, and all-target builds cover every member; the portable core compiles/tests without a Mac, CLI, or Swift runtime. Native checks cover the extracted mechanisms and their composition on macOS. Update source-path-sensitive native build/test tasks, executable metadata, and packaging references. Do not add empty networking, UIKit, or other future crates to illustrate the architecture; a later capability receives its own grounded ownership boundary when implemented.

Existing Rust-backed macOS capabilities continue to work without being rewritten into Swift. The AppKit native-read path becomes the first foreign-language implementation behind the action boundary. The first delivery explicitly assembles one implementation per exported action; no runtime provider-priority or fallback algorithm is introduced. A missing native action implementation is an inspection failure, not permission to substitute an accessibility tree.

### Language-neutral contract

The contract describes semantics rather than prescribing a Rust type layout. An extension exports named functions; the core invokes them by extension identity and exported name, not by a universal operation enum or fixed provider protocol. An export describes supported argument/result conversions, input/result schemas, execution location, applicable target/session requirements, prerequisites, bounds, and effects. Its assembled implementation has a build identity. Unexported functions remain ordinary implementation details.

The common boundary is Recon-compatible values, not arbitrary application structs that happen to serialize. Language SDKs provide native representations and explicit conversions for that contract. An author can choose a new function name without modifying an inspection interface; accepting a new type requires a supported conversion with truthful schema/error behavior. Wire framing and encoders stay below this author-facing boundary.

The first AppKit consumer exports the existing `windows`, `tree`, `node`, `find`, and `fonts` reads. These are concrete actions, not required members of every extension. Their bounds, fields, handles, error classifications, and observable outcomes remain those of proposal.0005. The existing open/reconcile/close access lifecycle stays runtime-owned rather than becoming author boilerplate. Discovery of an action does not claim it is currently usable: access may still be unavailable for the selected target.

All CLI routes continue to use one command registry for parsing, help, schemas, prerequisites, effects, and published errors. Generated extension metadata is not a separately maintained command catalogue. The draft leaves open whether newly exported actions receive explicit command mappings, generated registry entries, or a registry-defined generic invocation command; that choice must be resolved before implementation approval. Do not silently limit exportable actions to the five current AppKit reads to avoid making it.

For an action requiring a target session, the host routes the request only to the backend/session to which its identity belongs. Backend-local handles are opaque to the core. Display names, bundle identifiers, numeric equality with another backend's handle, and correlation results are not interchangeable authority. Existing AppKit session descriptors and public handle spellings remain compatible; this proposal does not rewrite them merely to advertise an internal backend identifier.

Wire encoding is private bridge machinery, distinct from the author-facing API. Preserve the current AppKit protocol version, framing, and operation spellings for its existing reads. Adapters connect those operations to generated action dispatch; the core action model must not inherit the native transport's fixed operation enum. The compiled action set remains closed to undeclared names, runtime-loaded code, selectors, and expressions supplied by a client. Extensible source declarations are not an arbitrary evaluation endpoint, and do not require embedding a Rust runtime in every target.

### Swift authoring SDK

Provide a Swift package under `sdks/swift/` with an author-facing `ReconExtension` module, macro support, and separate bridge/native-support internals. The generic module does not import AppKit or require native-inspection request types. `extensions/appkit/` uses it as a normal Swift package dependency and owns its domain-specific request/observation models and traversal.

#### Exported functions and Recon-compatible values

The author writes a normal Swift function, explicitly exports it under a name, and returns a Recon-compatible type or throws. There is no required enclosing provider instance, native-inspection protocol, or five-method conformance. An extension entry point supplies the namespace and assembled exports; macros generate descriptors and invocation/conversion adapters rather than make the author implement those adapters. Source-level export collection must be explicit and supported by Swift macros; do not assume a per-function macro can discover arbitrary declarations across a package.

The following is an illustrative authoring shape, not an existing SDK or a complete extension package:

```swift
@ReconAction("greeting")
func greeting(_ name: String) -> ReconValue {
    .string("Hello, \(name)")
}
```

`String` is a supported argument conversion; `ReconValue.string` constructs a Recon-compatible result. Normal Swift `throws` reports an operation failure, and a function may receive a runtime-supplied action context when it needs access scope, a deadline, or cancellation. That context is not a caller-supplied argument or a mandatory mutable handle for constructing every scalar. Shared implementation state may follow normal Swift ownership, but export does not require inventing a provider object.

The SDK defines the value and conversion boundary:

- A generic `ReconValue` represents supported null, Boolean, numeric, string, array, and object data. SDK collection/construction APIs compose these values without JSON strings, `[String: Any]`, or foreign pointers. The initial numeric, null/absence, nesting, and size rules must preserve the existing observation/wire contracts rather than silently lose precision or collapse unavailable information into null.
- Ordinary Swift scalar/collection types may be used where the SDK supplies a defined conversion. SDK observation, scoped-reference, completeness, and failure types preserve their existing distinctions. Swift names and APIs are designed for Swift; they are not bindings mechanically generated from Rust structs.
- An extension-defined domain model is compatible only through an explicit SDK conversion contract that also supplies the required schema information. `Codable` conformance alone does not export a type or establish Recon semantics. The exact conversion protocol and convenience macro spellings remain part of the pre-approval API proof.
- Returned data is an owned value representation suitable for bounded transport, not a JavaScript-GC handle or a live `NSView`, `NSWindow`, pointer, closure, or executable callback. Native references stay scoped to their owning backend/session and travel only through the existing opaque reference contract. This is a common value vocabulary, not a universal UI/object hierarchy.
- The runtime owns conversion validation, action/effect attribution, existing response/error envelopes, and wire encoding. AppKit functions construct their existing observation shapes through these APIs; returning a Recon value does not authorize changing the published payload or treating a malformed/unencodable result as success.

This is the Neon analogy: explicit exports plus native-language functions over host-compatible values and conversions. It does not copy JavaScript types, contexts, handle lifetimes, class exports, promises, or callback support into a CLI with a different execution and transport model. A greeting demonstrates the authoring shape; the migrated AppKit inspector supplies the real architectural proof.

#### Macros, isolation, and runtime access

Export names are independent of Swift function names. Generated descriptors/adapters use supported signatures and explicit metadata; effects and access prerequisites are declared rather than inferred from a function's name or body. One source-level export assembly feeds runtime discovery, not a second handwritten schema or registration manifest. Assembly rejects duplicate extension identifiers and duplicate exported names within a namespace, while permitting the same local name in different namespaces.

Macros operate on syntax and generated Swift must type-check. Unsupported function forms or argument/result conversions produce actionable author diagnostics. There is no implicit reflection over all public methods or automatic support for every Swift type. Pin export collection, annotation metadata, conversion/schema APIs, and supported signatures in the feasibility proof before approval rather than treating illustrative names as finalized API.

AppKit reads execute synchronously on `@MainActor` inside the target-resident helper, because they inspect live native state within a bounded traversal. Other functions may use different isolation or `async throws` where suspension is needed. Generated adapters preserve Swift isolation and error behavior instead of detaching work to bypass it or copying Neon's worker-pool choices.

The optional execution context exposes only the operation's applicable scope, deadline, and cancellation state. Native actions additionally use the AppKit/native-support module's typed reference facilities; unrelated functions do not inherit a native registry. Authentication secrets remain runtime-owned. The runtime owns queueing and rechecks access/deadline before dispatch; loops observe cancellation through the context, which cannot interrupt a getter already executing.

Normal Swift errors, target field-read failures, and access/lifecycle failures remain distinct. Missing/unavailable fields, truncated traversal, stale objects, and geometry retain current observation semantics through Recon-compatible values. The core never retains live framework objects. Swift ARC and actor isolation govern local implementation values; copying Neon's GC-handle model would not make Objective-C exceptions or cross-process references safe.

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

Behavioral verification derives from proposal.0005 and this section. Existing fixture coverage must run against the Swift implementation and demonstrate native fields/facets, bounds and incompleteness, weak-handle reuse/reparenting/staleness, process replacement rejection, queued-close races, timeout behavior, protocol/build mismatch rejection, and the no-focus contract. SDK tests register and invoke a test-only non-AppKit function through the same export/conversion path, without a provider object, core operation case, or special SDK method. Verify a concrete result and thrown failure, supported built-in and extension-defined value conversion, incompatible-type rejection, and preservation of numeric/null/completeness distinctions required by existing payloads. Reject undeclared exports and duplicate names within one namespace; permit the same local name in different namespaces. Check meaningful author diagnostics, not generated-source snapshots.

Use the ordinary Swift test tools for SDK/backend tests and the existing Rust/native integration tasks for end-to-end evidence. Add enforceable mise tasks for the new toolchain and checks. The repository gate remains `mise run check`; macOS-specific Swift and live inspection checks require documented macOS tasks/CI rather than pretending to run on the Linux gate. Required live evidence includes the currently supported cooperative and research paths; new research-target authorization must be obtained where needed.

Write the extension-author guide from this approved design before the implementation. It must explain exported functions, Recon-compatible types and conversions, export assembly, command exposure, optional context access, isolation, native exception limitations, cancellation, testing, and packaging with a real AppKit example. Explain what the Neon analogy does and does not promise. Do not describe the Swift SDK as available before it exists.

## Compatibility

The existing CLI grammar, help/schema source, success/error envelopes, observation field semantics, session modes, native handle semantics, no-focus behavior, and privacy identity remain unchanged for current commands. Existing AppKit actions back those same commands; moving code into crates or adding annotations does not rename the public surface. Stored native session descriptors keep their existing format. The exposure of additional named actions remains an explicit design question, not a promise of an unspecified new command family.

An updated helper build still requires a matching CLI and, when an older helper remains resident, the existing explicit target-restart workflow. There is no hot replacement, protocol downgrade, or old-backend fallback. Moving code to a Swift SDK does not expand supported host versions, target architectures, target access, or inspection capabilities.

Issue #74 is related work, not a prerequisite or authority for a simultaneous Rust-helper port. The recommendation is to defer its broad rewrite while reviewing this architecture. This draft does not close the issue, approve a replacement implementation, or alter historical proposal.0005. If this direction is accepted, update #74 to identify which remaining native runtime work is still relevant rather than maintaining competing production walkers.

## Implications on adoption

Recon users continue to install one product through the existing Homebrew path and use the same commands and grants. The build must package required runtime support; users must not install SwiftPM or an extension development environment to inspect an application from a bottle.

Contributors and source builds gain focused Rust capability/support crates, a pinned Swift toolchain, Swift package dependency resolution, SDK tests, and macOS build verification. The exact version floor and arm64e/runtime packaging evidence remain approval prerequisites. The Swift SDK is initially consumed from the same source revision, not published as an independently compatible package. There is no requirement to link unrelated inspectors into an extension merely because they support the same OS.

A normal release rollback can reinstall the previous matched artifact set; it cannot safely unload a newer helper from an already-running target. Target restart and privacy re-granting retain their existing meanings. Approval of this proposal does not itself authorize a package publication, release, or tap update.

## Scope

- Complete responsibility-level Rust workspace migration of existing application, WindowServer, accessibility, native-access, privacy, and no-focus implementations, with a portable core, explicit dependency direction, migrated callers/tests, and corrected build/packaging references.
- A Swift-native, Neon-inspired SDK for explicitly exported functions and Recon-compatible values/conversions, including generated adapters, necessary runtime/native support, and an independently owned AppKit consumer.
- Clean migration of the existing native observation path, with reproducible matched-artifact builds and the current supported target/access matrix preserved.
- Swift SDK author documentation and behavioral/native verification of both generic action authoring and the migrated AppKit path.

This is one integration change with one real consumer, not a commitment to implement every future backend. Crate extraction alone, an unused SDK, a macOS umbrella library, or a fixed AppKit provider interface with annotations added on top does not satisfy the proposed architecture. Focused crates are an ownership boundary, not a request for a crate per function or an empty package for every idea.

### Out of scope

- Designing or shipping additional language SDKs or new platform inspectors; those remain future work rather than approval prerequisites for this change.
- Replacing the Rust CLI with a native-language executable, or making a consumer-facing embedding API for unrelated applications.
- A public binary plugin ABI, dynamic third-party package loading, automatic extension discovery, untrusted-code sandboxing, or a marketplace.
- A universal UI tree, generic object evaluator, arbitrary selector invocation, new network capture, persistent evidence storage, streaming, or new user-facing inspection capabilities.
- Rewriting the C loader or assembly bootstrap, implementing #74 alongside this proposal, or deleting native runtime support merely to achieve language purity.
- Changing host security settings, signing requirements, permission ownership, or the no-focus policy to accommodate a new runtime.

## Preview

- Repository shape: focused Rust capability/support crates assembled into one CLI, plus a generic Swift SDK and separate AppKit implementation in the native helper; the proposal itself is rendered by the existing documentation site.
- Implementation artifact: a locally installable development CLI with its matched loader/helper/build-ID set, plus the source-consumable Swift package and AppKit extension from the same checkout. This is the cheapest realistic preview because a standalone SDK sample would not exercise target-side access, packaging, or the existing command contract.
- Exercise: use the fixture's existing native session/view commands through the preview executable and inspect concrete fields and stale-reference behavior. Modify one exported AppKit function and rerun its Swift tests and native fixture path. Exercise a non-AppKit SDK test export that accepts supported values and returns Recon-compatible data or throws, proving that authoring does not require native-inspection types. The readiness report supplies exact paths and commands actually exercised.
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
- A future networking capability should own its acquisition mechanism and domain models separately from window management, AX, and AppKit traversal, even when all run on macOS. The same applies to future binary, file, or browser inspectors. Those capabilities may share the value/export contract and narrow access services without becoming responsibilities of a platform-wide crate; no empty future crate is added by this proposal.
- Independent SDK releases, third-party extension installation, and additional hosting arrangements could be proposed once real consumers establish compatibility and trust requirements. They are not implied by source-level first-party extensibility.

## Alternatives considered

- **Rust-only implementation, including #74.** Offers one language for most logic and potentially shared protocol/lifecycle code. It does not provide the requested native-language authoring experience and retains native ABI/toolchain constraints. Individual Rust backends remain valid; Rust-only is not the proposed architectural policy.
- **Native-language CLIs embedding a Rust library.** Allows a native executable to own its runtime directly, but couples provider language to product assembly and adds embedding APIs without an established consumer. Native providers do not require replacing the current CLI.
- **Generate the public SDK from Rust declarations.** Reduces bridge maintenance, but makes Rust representations authoritative over Swift ergonomics. Generated adapters beneath a hand-designed authoring API remain an implementation option; generated API shape is not the product design.
- **Independent executable extensions for everything.** Gives a convenient language-neutral protocol and some host-side failure isolation, but does not remove target-resident inspection, adds process/lifecycle and permission-attribution concerns, and cannot isolate a target from code already injected into it. Use process boundaries where the mechanism needs them rather than mandate one deployment form.
- **A fixed native-inspection provider protocol.** Makes the first AppKit reads easy to enumerate, but confuses one consumer with the function-export contract. New actions would require central interface changes; renaming the protocol or generating its conformance does not remove that restriction.
- **A broad `recon-macos` extraction.** Makes the first move out of the CLI simple but leaves discovery, WindowServer, accessibility, target access, privacy, and guarding coupled by OS. It would become the default home for unrelated future networking and other capabilities rather than giving each a coherent owner.
- **Automatically serialize arbitrary Swift domain models.** Offers a familiar `Codable` workflow, but serialization alone does not define supported Recon values, schemas, observations, errors, or reference lifetimes. Explicit SDK conversions can support domain models without making arbitrary serialization the extension contract.
- **Neon-inspired function exports and values, with focused mechanism ownership.** Recommended. Ordinary Swift functions return host-compatible types through defined conversion rules; macros remove export glue, not choose the available capabilities. Narrow crates and a separate AppKit package give each mechanism an owner without copying Node's runtime or introducing a public binary plugin ecosystem.

## Open questions

- [NEEDS CLARIFICATION: Review the proposed responsibility-level crate partition and names. The recommendation separates applications, WindowServer/capture, accessibility, native access/sessions, privacy identity, and no-focus guarding, with AppKit traversal in its Swift package and no OS-wide facade. Confirm or revise the concrete ownership/dependency boundaries before implementation approval; future networking and other capabilities are not being implemented here.]
- [NEEDS CLARIFICATION: Choose how exported named actions become user-callable: explicit mappings to commands in the existing registry, generated command entries composed into that same registry, or a registry-defined generic invocation command. The SDK must not limit exportable actions to the existing AppKit reads, and help/schema/effect/error discovery must remain authoritative. Settle command naming and schema representation before approval.]
- [NEEDS CLARIFICATION: Confirm the recommended AppKit execution arrangement: a Swift backend inside the existing target-resident helper, retaining narrow C/Objective-C loader/runtime and exception-safe access support. A host-side Swift extension would be a different design and must establish privacy attribution and explain which native functionality it owns; it is not an equivalent substitute for a Swift AppKit inspector.]
- [NEEDS CLARIFICATION: Establish the pinned Swift toolchain and authorize the bounded feasibility proof before approval, or explicitly authorize it as an initial implementation gate. Record arm64/arm64e loading, runtime packaging, native exception containment, function-export assembly, Recon value/conversion/schema APIs, and actor-isolation evidence before treating the SDK/runtime API as settled. No such proof or additional research-target authorization exists in this draft.]
