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

Restructure Recon into focused, explicitly macOS-qualified implementation crates for application discovery, WindowServer operations, accessibility, native access, privacy identity, and no-focus guarding, alongside a portable execution core. Introduce a Swift-native extension SDK inspired by Neon: mark ordinary functions with `@ReconAction`, infer export names from their identifiers, and use Swift's Codable protocols as the payload serialization boundary. Prove the architecture with an independently owned AppKit inspector while preserving existing command, access, identity, and observation contracts.

## Motivation

Recon's owner wants inspection capabilities to be implemented in the language that best fits the inspected technology, starting with Swift for AppKit. Extension authors should expose ordinary functions over Codable payloads, not implement a provider interface, duplicate a function name in its export annotation, or manage a Rust ABI. Macros remove export and serialization plumbing without defining which kinds of inspection may exist.

The current Rust executable, Objective-C helper, and C loader already cross language and process boundaries, but those boundaries are private implementation details. There is no supported extension-authoring experience. Inside the Rust crate, application discovery, WindowServer capture, accessibility, native access, privacy, and UI guarding also share one platform module tree. Naming that entire collection `recon-macos` would preserve the coupling rather than establish focused ownership.

Issue [#74](https://github.com/markmals/recon/issues/74) proposes porting the native helper to Rust. Language uniformity could simplify parts of maintenance, but does not answer how different inspection technologies should integrate. Conversely, changing the executable's language would not by itself produce a reusable extension contract. The problem is the missing separation between CLI, shared investigation logic, platform integration, and extension authoring, not the language of `main`.

## Domain grounding

### Established context

- [Node-API](https://nodejs.org/api/n-api.html) is a deliberately designed native-addon API with binary compatibility guarantees across Node versions. Its separation of an authoring interface from implementation details is relevant; its independent binary-compatibility commitment is not a prerequisite for Recon.
- [Neon's function model](https://neon-rs.dev/docs/functions/) exports Rust functions that interact with a JavaScript host through a context and return JavaScript-compatible values or errors. Its [export macro](https://docs.neon-bindings.com/neon/attr.export) also supports ordinary Rust argument/result types through `TryFromJs` and `TryIntoJs` conversions. The relevant authoring model is functions plus supported host values and conversions, not a domain-specific provider interface.
- [Neon's value types](https://docs.neon-bindings.com/neon/types/) model JavaScript values; its [contexts and handles](https://docs.neon-bindings.com/neon/context/) manage access to a garbage-collected runtime. Recon is not a JavaScript engine. This proposal takes the export/conversion ergonomics as precedent, not V8-style object handles, JavaScript inheritance, Node's worker pool, or an in-process Node addon ABI.
- [UniFFI](https://mozilla.github.io/uniffi-rs/latest/) generates foreign-language bindings to Rust libraries. Binding generation can implement a bridge, but does not establish an idiomatic extension API or its lifecycle semantics.
- Swift's [attached macros](https://github.com/swiftlang/swift-evolution/blob/main/proposals/0389-attached-macros.md) can generate wrapper functions, peer declarations, and members from annotated declarations. [Swift Package Manager supports macro targets](https://www.swift.org/blog/swift-5.9-released/#swift-package-manager). This supports explicit function exports without requiring an SDK protocol for every possible action. Macro expansion works from syntax; generated code must still type-check, preserve actor isolation, and use supported value encodings. Macros do not inherently provide runtime reflection, another process's main thread, or a security boundary.
- Swift's [Codable protocols](https://developer.apple.com/documentation/swift/codable) combine `Decodable` and `Encodable`. [Apple's custom-type guidance](https://developer.apple.com/documentation/foundation/encoding-and-decoding-custom-types) describes synthesized conformances, `CodingKeys`, and custom implementations that may change a payload's shape. Codable supplies serialization, not a discovery-schema API. A Swift syntax macro cannot infer every custom codec's complete schema from its function signature; schema derivation and custom-codec support need their own explicit design.
- Apple's [thread-safety guidance](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html) constrains AppKit view access. The existing inspector runs in the target process and dispatches native reads to that application's main thread. A Swift host executable or host-side `@MainActor` annotation cannot replace target-side execution.
- Apple explicitly states that [Objective-C exceptions must be caught before reaching Swift code](https://developer.apple.com/documentation/swift/handling-cocoa-errors-in-swift). Swift `throws` is not an Objective-C exception boundary. Wrapping a Swift callback in Objective-C `@try` would still permit an exception to unwind through Swift frames and is not an acceptable solution.
- [Windows UI Automation](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-uiautomationoverview) is a COM-based accessibility framework, not the actual WinUI visual tree. [Electron's debugger API](https://www.electronjs.org/docs/latest/api/debugger) supplies a Chrome DevTools Protocol transport for a renderer. Framework, access mechanism, language, and evidence source are independent choices.

### Relevant constraints and principles

One Recon product can use several implementation languages and several inspection mechanisms on the same operating system. An extension may need host code, target-resident code, or a protocol client; an SDK must not force all three into one binary-plugin model. Crate boundaries follow coherent responsibilities and dependency direction. Concrete platform implementations identify their OS in their names; an unqualified capability crate must own an actual shared contract or portable behavior, not imply unimplemented platform support. A crate, Swift package, extension, and deployed process need not correspond one-to-one.

The authoring boundary is a normal function with Decodable inputs and an Encodable result; models used in both directions can adopt Codable. Authors use Swift types, `throws`, and actor isolation rather than constructing a required foreign-value wrapper or translating Rust ownership conventions. Serialization and schema/effect/access semantics are distinct responsibilities. Generated plumbing is acceptable; mechanically generated Rust-shaped APIs and fixed AppKit method lists are not. Transport may be hidden, but execution location, access prerequisites, and irreversible effects remain visible.

### Ontology review

**CONCEPT INVENTORY**

- **Host** — the environment running Recon's executable.
- **Target environment** — where the inspected subject exists, such as the host, a simulator, or a connected device.
- **Target** — the selected process incarnation, page, or artifact; its identity comes from its own domain rather than a universal PID or bundle path.
- **Capability** — an inspection or control behavior Recon can offer; it is not a mandatory SDK method or a fixed list of UI operations.
- **Action** — an explicitly annotated function whose identifier supplies its export name within an extension, with typed serialized inputs/outcomes, prerequisites, execution location, and effects.
- **Backend** — the implementation of a capability through a particular inspection mechanism.
- **Extension** — the distributable implementation exporting named actions to Recon; it can contain more than one backend or execution component.
- **Language SDK** — the extension-author-facing library, compiler support, and testing facilities for one language.
- **Serialized payload** — the bounded data representation of an action's input or result; Swift types participate through Decodable/Encodable, without exporting live native-object identity or requiring a universal value-wrapper type.
- **Session** — an access relationship to a particular target, not the extension process, loaded code, or a saved observation.

**CONFLATIONS AND RELATIONSHIPS**

AppKit and browser runtimes describe useful inspection areas, not mutually exclusive application classes. An Electron application can expose native windows, accessibility elements, DOM nodes, and network observations. Several backends may inspect the same target; one capability may be exposed through several functions. A crate groups one coherent implementation responsibility, not everything available on an OS. The AppKit extension owns NSView traversal independently of WindowServer and accessibility. An action is not inherently a UI read, and an action that does not inspect a live target need not invent a target session. A CLI command is a user-facing route to an action, not the action's identity or its Swift function name.

**IDENTITY / LIFECYCLE ISSUES**

Extension/action identity, backend identity, target identity, session identity, native object handles, and immutable observations remain distinct. An action's identity is its extension identifier plus its annotated function's base name. Renaming that function changes the export and is a contract change, not an invisible implementation refactor. Overloaded annotated functions cannot share one external name in the same extension. A process restart or page replacement cannot be hidden by reusing a display name. Closing access does not imply unloading target-resident code. Native references remain meaningful only within their owning backend/session; correlation does not make them interchangeable.

**ASSESSMENT: COHERENT with these distinctions.** The contract is exported functions over Codable payloads, not a universal provider protocol or a platform-wide backend. Serialization does not by itself establish a discovery schema, observation meaning, or native-reference authority. Platform-qualified implementation crates do not claim to define generic capability contracts. Concrete crate boundaries, command exposure, and deployment remain subject to human review.

### Quality bar

An extension author marks a function with `@ReconAction`, accepts Decodable Swift inputs, returns an Encodable result or throws, and tests it with normal Swift tools. They do not repeat the export name, implement unrelated methods, edit a core operation enum, construct wire messages, or adopt a Recon-specific conversion protocol merely to serialize a domain model. The generic SDK works without AppKit. Standard Codable models should have generated schema support without a second handwritten catalogue; custom codec shapes need an explicit schema path, not guessed metadata.

The resulting CLI retains source-attributed observations, bounded work, truthful failure and cancellation reporting, and the existing no-focus and privacy guarantees. A demonstration that only serializes an example value is not evidence that the extension architecture works.

### Remaining uncertainty

The Rust CLI/core, inferred export naming, and Codable serialization direction are established, not a finalized schema-generation API or a verified target-resident runtime. The focused, macOS-qualified crate partition below is a draft design, not a permanent count. Crate ownership, command exposure, AppKit execution location, and feasibility evidence remain explicit review questions. In particular, Codable conformance is not evidence that arbitrary custom codecs already have accurate discovery schemas. No Swift/arm64e loading or native SDK prototype was exercised while authoring this proposal.

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

Make responsibility-level decomposition and the function/payload extension boundary the two connected parts of this change. Retain one Rust CLI and portable `recon-core`; extract current platform implementations into focused `-macos` crates rather than an OS-wide facade or misleading generic capability libraries. The Swift SDK takes Neon's export ergonomics into Swift: bare `@ReconAction`, inferred function names, Codable payloads, optional execution context, and generated invocation/serialization adapters.

The recommended first consumer is Recon's existing AppKit native inspector. It must perform real inspection through the SDK and replace the previous traversal implementation at cutover. Existing C/Objective-C runtime support may remain where it supplies the loader, authentication, lifecycle, or exception-containment boundary; extension authors do not write or call its C interface directly.

```text
Host-side product assembly

recon
  ├─ recon-core
  ├─ recon-applications-macos
  ├─ recon-window-server-macos
  ├─ recon-accessibility-macos
  ├─ recon-native-access-macos ── authenticated transport ── target helper
  ├─ recon-privacy-macos
  └─ recon-ui-guard-macos

Target helper: Swift SDK runtime + AppKit extension + native support
```

The tree shows the CLI's assembled dependencies, not a requirement that every capability depend on every other one. Platform implementation crates use the portable core contracts and only the focused services they require; the core imports none of them. The transport line is a process boundary, not a Cargo dependency. The Swift AppKit package remains separate from `recon-native-access-macos`. No Rust core is injected merely to support Swift.

## Detailed design

The following specifies the recommended draft arrangement. Open questions identify the choices and feasibility evidence that must be resolved before the human approves implementation; this draft is not permission to silently select a different scope.

### Rust workspace rearchitecture

Extract current implementations along the following focused ownership boundaries. Concrete macOS crates use the `recon-<responsibility>-macos` convention; their names state their actual support rather than imply a portable implementation. These are draft boundaries for existing behavior, not an exhaustive taxonomy or a permanent crate count:

| Crate | Responsibility |
| --- | --- |
| `recon` | Executable entry point, argument parsing, one command registry, help/schema presentation, product assembly, standard-stream delivery, exit behavior, and embedded executable build/privacy metadata |
| `recon-core` | Portable action identities, export descriptors, shared serialized-payload/observation/error contracts, and invocation orchestration; no platform I/O or capability-specific model catalogue |
| `recon-applications-macos` | macOS bundle metadata, indexed/running application discovery, target resolution, and existing launch behavior; not a universal application model |
| `recon-window-server-macos` | macOS WindowServer/SkyLight window inventory and capture, including their private native calls; no AX tree or NSView traversal |
| `recon-accessibility-macos` | macOS AX element access, attributes, window-element resolution, menu handling, and existing tree traversal/model; not a cross-platform accessibility contract |
| `recon-native-access-macos` | macOS process-incarnation-bound helper access: loader integration, build matching, research-access prerequisites, authenticated sessions, persistence, bounded transport, and open/reconcile/close lifecycle; no NSView walker |
| `recon-privacy-macos` | Executable-owned macOS privacy identity, signing/permission facts and requests, responsibility re-execution, and the child-output/signal behavior that makes that boundary truthful |
| `recon-ui-guard-macos` | macOS continuous no-focus monitoring, scope/effect attribution, and interference classification shared by UI mechanisms; not a platform-neutral UI policy implementation |

An unqualified capability crate such as `recon-accessibility` is appropriate only if it owns a real platform-neutral trait/model contract or portable behavior that callers and the concrete backend actually use. This delivery recommends the concrete `-macos` crates rather than inventing generic interfaces to justify generic names. A later shared contract may be extracted when its semantics and consumer need are established; it must not be an empty facade or re-export of one platform implementation. `recon-core` remains unqualified because its shared invocation/payload contracts are genuinely platform-independent.

Each crate lives under `crates/<crate-name>/`. `recon` assembles the product and depends on the capability/support crates only on supported hosts. Those crates depend inward on `recon-core`, never on the executable. A capability may use a focused service where it needs one, but WindowServer, accessibility, and AppKit observation do not require one another merely because they run on macOS. Keep the dependency graph acyclic. Low-level monitoring primitives used by the guard must not invoke a guarded inspector and recurse through product dispatch.

The generic core stays small: portable invocation, serialized-payload, and error semantics belong there, not every platform-independent request struct. Capability-specific models and pure traversal logic move with their concrete capability unless an actual shared contract is extracted. Swift AppKit lives in `extensions/appkit/`, with native-access support separated from the generic SDK. It is not a Rust placeholder or part of the WindowServer/AX crates. The winctl-inspired WindowServer operations, AX observations, and NSView observations remain distinct evidence sources; this extraction adds no new UI input.

This is migration of implementations, not new facades around the old library. Split the current `macos/ui/` modules among their owners; move the relevant `application/` and `ui/` models with their capabilities; move `inspection/`, `macos/inject/`, `inspection_access.rs`, and native-session/artifact integration into native access. Keep CLI parsing/rendering in `recon`. Context reporting composes facts from the owning crates rather than becoming an OS-wide implementation library. AX/native correlation belongs in product composition over the two sources; it must not make either inspector own the other.

Migrate every import and caller to its owner. Behavioral tests move with the responsibility they specify; executable contract and multi-mechanism integration tests exercise the assembled product. Do not retain an umbrella platform crate, forwarding modules, dead re-exports, duplicate dispatch paths, or a compatibility library preserving the former ownership. Preserve unsupported-host reporting, executable identity, native artifact names, privacy grants, and continuous guarded dispatch throughout extraction. Splitting crates must not create a command path around the guard or the privacy boundary.

Register every real crate in the workspace with the shared version and lint policy. `mise run check`, rustdoc, and all-target builds cover every member; the portable core compiles/tests without a Mac, CLI, or Swift runtime. Native checks cover the extracted mechanisms and their composition on macOS. Update source-path-sensitive native build/test tasks, executable metadata, and packaging references. Do not add empty networking, UIKit, or other future crates to illustrate the architecture; a later capability receives its own grounded ownership boundary when implemented.

Existing Rust-backed macOS capabilities continue to work without being rewritten into Swift. The AppKit native-read path becomes the first foreign-language implementation behind the action boundary. The first delivery explicitly assembles one implementation per exported action; no runtime provider-priority or fallback algorithm is introduced. A missing native action implementation is an inspection failure, not permission to substitute an accessibility tree.

### Language-neutral contract

The contract describes semantics rather than prescribing a Rust type layout. An extension exports annotated functions; the core invokes them by extension identifier and inferred function name, not by a universal operation enum or fixed provider protocol. An export describes typed serialized inputs/results, discovery schemas, execution location, applicable target/session requirements, prerequisites, bounds, effects, and build identity. Unannotated functions remain implementation details.

The Swift boundary is the Codable protocol family: generated adapters decode supplied payloads into Decodable parameters and encode Encodable results. Full Codable conformance is convenient for models used in both directions, but the runtime requires only the direction it actually uses. No additional Recon-value wrapper or conversion protocol is required for ordinary Codable models. Other language SDKs may express the same serialized contract idiomatically; the shared core does not depend on Swift protocols.

Codable is the serialization boundary, not the whole action contract. The SDK still needs accurate schemas, bounded messages, explicit effects/access, and current observation/error semantics. Custom codecs remain Codable; supporting them does not justify guessing their schema from stored properties or silently changing wire representation.

The first AppKit consumer exports the existing `windows`, `tree`, `node`, `find`, and `fonts` reads. These are concrete actions, not required members of every extension. Their bounds, fields, handles, error classifications, and observable outcomes remain those of proposal.0005. The existing open/reconcile/close access lifecycle stays runtime-owned rather than becoming author boilerplate. Discovery of an action does not claim it is currently usable: access may still be unavailable for the selected target.

All CLI routes continue to use one command registry for parsing, help, schemas, prerequisites, effects, and published errors. Generated extension metadata is not a separately maintained command catalogue. The draft leaves open whether newly exported actions receive explicit command mappings, generated registry entries, or a registry-defined generic invocation command; that choice must be resolved before implementation approval. Do not silently limit exportable actions to the five current AppKit reads to avoid making it.

For an action requiring a target session, the host routes the request only to the backend/session to which its identity belongs. Backend-local handles are opaque to the core. Display names, bundle identifiers, numeric equality with another backend's handle, and correlation results are not interchangeable authority. Existing AppKit session descriptors and public handle spellings remain compatible; this proposal does not rewrite them merely to advertise an internal backend identifier.

Wire encoding is private bridge machinery, distinct from the author-facing API. Preserve the current AppKit protocol version, framing, and operation spellings for its existing reads. Adapters connect those operations to generated action dispatch; the core action model must not inherit the native transport's fixed operation enum. The compiled action set remains closed to undeclared names, runtime-loaded code, selectors, and expressions supplied by a client. Extensible source declarations are not an arbitrary evaluation endpoint, and do not require embedding a Rust runtime in every target.

### Swift authoring SDK

Provide a Swift package under `sdks/swift/` with an author-facing `ReconExtension` module, macro support, and separate bridge/native-support internals. The generic module does not import AppKit or require native-inspection request types. `extensions/appkit/` uses it as a normal Swift package dependency and owns its domain-specific request/observation models and traversal.

#### Exported functions and Codable payloads

The author annotates a normal function with bare `@ReconAction`. Its Swift base identifier becomes the export name exactly, without repeating a string or silently changing naming conventions. There is no required provider instance or fixed method conformance. An extension entry point supplies the namespace and assembled exports; macros generate descriptors and decoding/invocation/encoding adapters. Source-level collection must be explicit and supported by Swift macros, not assumed package-wide reflection.

The following is an illustrative authoring shape, not an existing SDK or a complete extension package:

```swift
struct Greeting: Codable {
    let message: String
}

@ReconAction
func greeting(_ name: String) -> Greeting {
    Greeting(message: "Hello, \(name)")
}
```

The function is exported as `greeting`; `String` supplies input decoding and `Greeting` supplies ordinary Codable output. No `ReconValue` construction or extra conversion conformance is needed. A scalar, collection, or domain model can participate through the same protocol boundary. Normal `throws` reports an operation failure; an optional runtime-supplied context provides access scope, deadlines, and cancellation when needed. Context is not caller-supplied serialized input, and exporting a function does not require a provider object.

The SDK owns the serialization/runtime boundary:

- Validate supplied input against its published contract and decode it through Decodable before invoking the function; decoder defaults must not silently override schema constraints such as unknown-key handling. Encode results through Encodable. SDK-owned codec settings preserve current field spellings, numeric precision, enum representation, null/absence distinctions, and response bounds. Models may use synthesis, `CodingKeys`, or custom `init(from:)`/`encode(to:)`; no universal SDK value tree is required.
- Keep data models separate from live framework objects. Observation, completeness, failure, and opaque-reference types may themselves be Codable, but their existing semantics and backend/session scope remain binding. Serializable data is not a transported native object, pointer, closure, or executable callback.
- Fail malformed/undecodable input before invoking the function body with the existing invalid-input classification. A result that cannot be encoded or exceeds the published payload contract is an explicit bounded failure, not null, an empty observation, a stringified value, or a reason to run the action again. Operation failures retain the current typed classification rather than requiring every thrown Swift error to be Codable.
- Encoding and decoding, including custom codec bodies, are extension execution under the same applicable isolation, access, deadline, and native-exception constraints. Codable is not a sandbox and does not make an Objective-C getter safe. Complete result encoding while the required execution scope is valid; closing access still must not free objects used by in-flight work.
- The runtime retains action/effect attribution, response/error envelopes, framing, and transport. Preserve the existing AppKit operation payloads and avoid a new mandatory encode-to-string/parse-to-value/encode-again layer merely to use Codable.

#### Discovery schemas are separate from serialization

Codable specifies how a type reads or writes an external representation; it has no requirement that enumerates the schema of every possible representation. Generated discovery for supported synthesized models should follow their actual serialized shape, including supported `CodingKeys` and optionality, not just Swift property spellings. A custom codec may encode a struct as a scalar or a differently nested object, so generic reflection or one sample encoding cannot establish its schema.

The SDK design must provide a generated schema path for the supported ordinary Codable model forms and a truthful explicit schema path for custom forms it cannot derive. Keep that metadata attached to the action/model export and composed into the existing command registry, not a second standalone command catalogue. Exact generation/annotation mechanics and custom-codec coverage remain approval prerequisites in Open questions. Do not claim all Codable types automatically have schemas, silently publish an inaccurate schema, or replace the Codable serialization boundary with a new conversion protocol to evade this requirement.

This preserves the Neon analogy—explicit exports, ordinary native-language functions, host-compatible data, and generated bridge plumbing—while using Swift's native serialization protocols. It does not copy JavaScript types, GC-handle lifetimes, class exports, promises, or callbacks into a CLI with a different runtime. The greeting demonstrates authoring, not architectural feasibility; the migrated AppKit inspector remains the real proof.

#### Macros, isolation, and runtime access

Bare `@ReconAction` infers the function's base name. Renaming it changes the exported action identity; parameter labels and overload signatures do not create additional external namespaces. Assembly rejects duplicate extension identifiers and duplicate inferred names within an extension, including annotated overloads, while allowing the same local name in distinct extensions. Invalid external names produce source-local diagnostics rather than silent rewriting.

Generated descriptors/adapters use the typed signature and explicit action metadata; effects and access prerequisites are not inferred from a function's name or body. Macro expansion is syntactic and generated code must type-check, including Decodable/Encodable requirements. There is no implicit export of all public methods, arbitrary schema reflection, or promise of support for every Swift signature. Prove export assembly, schema derivation, custom codec support, metadata, and supported signatures before treating the API as finalized.

AppKit reads execute synchronously on `@MainActor` inside the target-resident helper, because they inspect live native state within a bounded traversal. Other functions may use different isolation or `async throws` where suspension is needed. Generated adapters preserve Swift isolation and error behavior instead of detaching work to bypass it or copying Neon's worker-pool choices.

The optional execution context exposes only the operation's applicable scope, deadline, and cancellation state. Native actions additionally use the AppKit/native-support module's typed reference facilities; unrelated functions do not inherit a native registry. Authentication secrets remain runtime-owned. The runtime owns queueing and rechecks access/deadline before dispatch; loops observe cancellation through the context, which cannot interrupt a getter already executing.

Normal Swift errors, target field-read failures, and access/lifecycle failures remain distinct. Missing/unavailable fields, truncated traversal, stale objects, and geometry retain current observation semantics in Codable models and their serialized payloads. The core never retains live framework objects. Swift ARC and actor isolation govern local implementation values; neither Codable nor a copied GC-handle model makes Objective-C exceptions or cross-process references safe.

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

Behavioral verification derives from proposal.0005 and this section. Existing fixture coverage must run against the Swift implementation and demonstrate native fields/facets, bounds and incompleteness, weak-handle reuse/reparenting/staleness, process replacement rejection, queued-close races, timeout behavior, protocol/build mismatch rejection, and no-focus behavior. SDK tests register and invoke a bare-annotated non-AppKit function with inferred naming and a Codable domain result, without a provider object, core operation case, or extra conversion protocol. Verify concrete results and thrown failures, invalid input rejected before the function body, encoding failure without re-execution, and preservation of numeric/null/completeness semantics. Cover supported synthesized models, renamed `CodingKeys`, custom codecs with accurate schemas, and serialization/schema mismatches rejected rather than advertised as valid. Reject undeclared exports and duplicate inferred names/annotated overloads within a namespace; permit equal local names across extensions. Check meaningful diagnostics, not generated-source snapshots.

Use the ordinary Swift test tools for SDK/backend tests and the existing Rust/native integration tasks for end-to-end evidence. Add enforceable mise tasks for the new toolchain and checks. The repository gate remains `mise run check`; macOS-specific Swift and live inspection checks require documented macOS tasks/CI rather than pretending to run on the Linux gate. Required live evidence includes the currently supported cooperative and research paths; new research-target authorization must be obtained where needed.

Write the extension-author guide from this approved design before implementation. Explain bare action annotations and inferred names, Decodable inputs/Encodable results, Codable domain models, synthesized/custom codecs and schema metadata, export assembly, command exposure, optional contexts, isolation, native exceptions, cancellation, tests, and packaging with a real AppKit example. Explain the Neon analogy and its limits. Do not describe the SDK as available before it exists.

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
- A Swift-native, Neon-inspired SDK for bare-annotated functions with inferred export names and a Codable-family serialization boundary, including accurate schema metadata, generated adapters, necessary runtime/native support, and an independently owned AppKit consumer.
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
- Exercise: use existing native session/view commands against the fixture and inspect concrete fields and stale-reference behavior. Modify one bare-annotated AppKit function and rerun its Swift/native checks. Exercise a non-AppKit test export with an inferred name, Decodable inputs, and an Encodable domain result or thrown failure; inspect the corresponding discovery schema against the actual serialized payload. The readiness report supplies exact commands and paths actually exercised.
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
- **A mandatory Recon-value/conversion wrapper.** Makes the bridge representation explicit but adds a second type/conformance vocabulary for payloads that Swift already models through Codable. It is not required for ordinary data models.
- **Treat Codable as automatic schema discovery.** Serialization is idiomatic, but custom codec bodies can change representation in ways a function annotation cannot infer. Use Codable for the data boundary and resolve schema generation/custom metadata separately.
- **Repeat the function name in every annotation.** Makes the export spelling explicit but duplicates the normal identifier without adding information. Bare annotations infer the name; duplicate/invalid names are diagnosed.
- **Generic capability names for macOS-only code.** Hide the current implementation boundary. Use `-macos` for concrete implementations; introduce an unqualified capability crate only when it owns an actual shared trait/model contract or portable behavior.
- **Neon-inspired exports with Codable and platform-qualified ownership.** Recommended. Ordinary Swift functions use their own names and data models; macros remove export and codec glue, while schema metadata remains truthful. Focused implementation crates state their actual platform without inventing speculative contract facades.

## Open questions

- [NEEDS CLARIFICATION: Review the proposed focused `recon-<responsibility>-macos` implementation crates and their concrete dependency boundaries. Unqualified capability crates would own real portable trait/model contracts, not macOS implementations; this delivery recommends suffixed implementations alongside the portable core. AppKit traversal stays in its Swift package, and future networking is not being implemented.]
- [NEEDS CLARIFICATION: Choose how exported named actions become user-callable: explicit mappings to commands in the existing registry, generated command entries composed into that same registry, or a registry-defined generic invocation command. The SDK must not limit exportable actions to the existing AppKit reads, and help/schema/effect/error discovery must remain authoritative. Settle command naming and schema representation before approval.]
- [NEEDS CLARIFICATION: Confirm the recommended AppKit execution arrangement: a Swift backend inside the existing target-resident helper, retaining narrow C/Objective-C loader/runtime and exception-safe access support. A host-side Swift extension would be a different design and must establish privacy attribution and explain which native functionality it owns; it is not an equivalent substitute for a Swift AppKit inspector.]
- [NEEDS CLARIFICATION: Establish the pinned Swift toolchain and authorize the bounded feasibility proof before approval, or explicitly authorize it as an initial implementation gate. Record arm64/arm64e loading, packaging, native exception containment, bare-annotation export assembly, Codable encoding/decoding, schema generation for supported synthesized models, explicit schema support for custom codecs, and actor isolation. Resolve the concrete schema mechanism and supported model forms before treating the API as settled; no such proof or additional research-target authorization exists yet.]
