---
id: proposal.0005
title: Native macOS Inspection
authors: [Mark Malstrom]
status: draft
pull-request: https://github.com/markmals/recon/pull/54
issues: ["#51", "#44"]
supersedes: []
---

# Native macOS Inspection

## Summary

Inspect a running macOS application's actual windows, views, cells, and layers through an explicit in-process session. Support cooperative access and research-machine attachment, bounded structural queries, fonts and layout, and correlation with separate Accessibility evidence. Report access limits and inspection effects without changing platform protections.

## Authorization

- Authority: On 2026-10-27, Mark said, “Let's do it! Create a new branch/worktree and write up a proposal!” after the recommendation to deliver macOS inspection sessions (#51) with native view/layer inspection (#44) as their first consumer. Evidence: Sesh session `b93eeb2e-701c-4407-b6f0-05e6649d36eb`; workspace [PR #54](https://github.com/markmals/recon/pull/54).
- Fit: One end-to-end capability: select a macOS process, establish supported access, inspect its native visual implementation, and end the session. The session machinery is independently verifiable but is not the feature's only usable result.
- Decisions recorded: In that same session, Mark selected **Research-machine access too** rather than cooperative-only access, and **Current Mac first** rather than older macOS or Intel support. The required platform is Apple silicon on macOS 27. Research access is required work, not a future direction or permission to close the issues with only a cooperative fixture working.
- Limits: Consent covers proposal development and its branch/draft PR. It does not approve implementation, merge, release, spending, elevated execution, signing or entitlement changes to other apps, or changes to this Mac's security or permissions. No build mandate is being exercised. The design remains draft while the questions below are unresolved.

## Motivation

Recon's current `ui tree` observes an accessibility hierarchy. That hierarchy deliberately hides implementation detail: an accessible button need not reveal its cell, surrounding views, backing layers, typography, constraints, or visual-effect material. A screenshot reveals the pixels but not the objects that produced them. An agent investigating a private subclass, native layout, or a SwiftUI application's AppKit realization cannot answer those questions with either observation alone.

The desired subject is a running application's implementation, including applications the investigator did not build. Requiring every target to adopt an SDK would exclude an important part of that work. Conversely, treating all installed applications as injectable would conceal real signing, debugger, sandbox, pointer-authentication, and system-protection boundaries. Researchers need a useful native inspector that makes those distinctions explicit.

The present workaround is to combine Recon's external observations with a separate in-process tool or debugger and manually reconcile process identity, windows, coordinates, and observations. #51 and #44 put that investigation behind Recon's existing targeting, discovery, and error contracts without redefining Accessibility as a native tree.

## Domain grounding

### Established context

- Apple's [Accessibility model](https://developer.apple.com/library/archive/documentation/Accessibility/Conceptual/AccessibilityMacOSX/OSXAXmodel.html) explicitly permits an app to omit implementation-specific objects such as content views and cells. AX elements and native objects therefore have neither equal membership nor a guaranteed one-to-one relationship.
- [`NSView.subviews`](https://developer.apple.com/documentation/appkit/nsview/subviews) describes actual containment and ordering; the array can change while the application runs. A child's index, class, name, or current frame is not its object identity. [`frame`](https://developer.apple.com/documentation/appkit/nsview/frame) is in the superview's coordinate system; `bounds` is in the view's own. A flipped parent prevents a universal “every view frame is bottom-left” interpretation.
- A view may have a [`layer`](https://developer.apple.com/documentation/appkit/nsview/layer), but layer backing and [layer hosting](https://developer.apple.com/library/archive/documentation/GraphicsAnimation/Conceptual/HighResolutionOSX/CapturingScreenContents/CapturingScreenContents.html) differ. `CALayer.sublayers` is not `NSView.subviews`. A [presentation layer](<https://developer.apple.com/documentation/quartzcore/calayer/presentation()>) approximates current on-screen animation state; a model-layer read does not.
- [`NSLayoutGuide`](https://developer.apple.com/documentation/appkit/nslayoutguide) participates in constraints without being a view. [`NSView.constraints`](https://developer.apple.com/documentation/appkit/nsview/constraints) returns constraints held by that view, not every constraint affecting it. Constraint ownership and constrained items are different relationships.
- [`NSHostingView`](https://developer.apple.com/documentation/swiftui/nshostingview) is an AppKit bridge that hosts a SwiftUI hierarchy. Walking its AppKit descendants does not recover every SwiftUI `View` value, modifier, or AttributeGraph relationship.
- AppKit's [thread-safety guidance](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html) restricts view operations to the application's main thread. Reading an overridable property executes target code. An Objective-C exception boundary does not recover a deadlocked getter or guarantee recovery from invalid native memory access.
- Apple's [debugger entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.debugger) documentation distinguishes the inspector's debugger rights from the target's `get-task-allow` entitlement and describes possible administrator authorization. Accessibility permission is not a task port or permission to load code.
- [Hardened Runtime](https://developer.apple.com/documentation/security/hardened-runtime), [library validation](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.disable-library-validation), and [DYLD environment-variable handling](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.cs.allow-dyld-environment-variables) impose separate constraints. Loaded libraries inherit the host executable's entitlements. A signable helper or a single disabled protection does not establish that a particular target can be inspected.
- Maintained injection machinery has separate arm64e concerns. Frida documents [macOS debugger authorization](https://frida.re/docs/examples/macos/), [arm64e system-process support fixes](https://frida.re/news/2022/10/22/frida-16-0-2-released/), and later [pointer-authentication and loader fixes](https://frida.re/news/2026/07/17/frida-17-16-0-released/). These establish relevant prior engineering, not compatibility with every macOS 27 target.

### Relevant constraints and principles

There are six distinct things: an application bundle, a running process incarnation, an inspection session, a native object, an immutable observation, and an AX element reported through a separate interface. A bundle can have several processes; a process can have successive sessions; an object can move between parents without becoming another object; an observation survives all of them as historical evidence. A session handle or native pointer is not a durable evidence identifier.

Cooperation, target architecture, signing/protection facts, and host security posture are independent dimensions. “Research” describes an explicitly selected access profile on an operator-prepared machine, not a target class, an entitlement, or an assertion that all protections are absent. arm64e is a relevant Apple-silicon target ABI, not Intel/Rosetta support; it cannot be silently excluded from the protected-system-app outcome.

### Quality bar

A useful result identifies the exact process and observation interval, preserves native relationships and coordinate spaces, narrows work before expensive reads, distinguishes absence from unavailable data, and never resolves an old handle to a replacement object. A successful session proves real target-side execution. A complete feature demonstrates both cooperative and research access; an honest denial on a stock machine is necessary but not a substitute for the latter.

### Remaining uncertainty

The controlled launch path below is observed. Live attachment, a macOS 27 arm64e loader, and protected-target access on a research machine are not. The current Mac reports SIP enabled and empty `kern.bootargs`; no alternate research host or target set has been authorized in this conversation. Neither that fact nor earlier project's documentation authorizes changing the machine. The loader selection and research validation matrix remain explicit approval blockers in Open questions.

## Existing baseline

**Recon, observed:** At base `fa4350d`, `cargo run -p recon -- --help`, `schema`, and `schema ui tree` expose application discovery, explicit launch, external AX/window observation, screenshot capture, command effects, and typed JSON errors. There is no native inspection session or native-tree command. Preserve exact-bundle/running-instance selection, the registry, one-object JSON responses, and the existing privacy identity. Existing `ui` behavior must not acquire an implicit injection dependency.

**apple-platform-tools, exercised:** Revision [`3e587d4f0f7a5c771234fdf52ac409b6d58a65ff`](https://github.com/markmals/apple-platform-tools/tree/3e587d4f0f7a5c771234fdf52ac409b6d58a65ff) was built unchanged in an isolated scratch directory with Swift 6.4 on arm64 macOS 27.0.1, build 26A434. The actual commands were `swift build --scratch-path "$SCRATCH/build" --product uitool`, then the same build for `UIToolBoot` and `SampleAppKitApp`. Using the resulting executables:

```text
uitool launch <scratch-build>/SampleAppKitApp
uitool windows <returned-pid>
uitool tree <returned-pid> --at <session>:w0/cv --depth 3 --include constraints,layer --limit 50
uitool node <returned-pid> --at <returned-greeting-node> --include class,frame,constraints,layer
uitool tree <returned-pid> --at <session>:w0/cv --depth 0 --fields node,class,childCount
uitool signing <scratch-build>/SampleAppKitApp
uitool detach <returned-pid>
uitool node <returned-pid> --at <previous-greeting-node>
```

The launch completed a real in-process handshake. Window reads returned an `NSWindow` and `NSPanel`; the depth-three tree returned seven view records, including a private AppKit label descendant, an `NSVisualEffectView`, and nested private `CALayer` subclasses. The greeting was an `NSTextField` with a 13-point system font, frame `(130, 260, 200, 20)`, and four reported touching constraints. Depth zero returned only the selected root, with four children reported. Signing inspection reported an ad-hoc, non-hardened fixture with `get-task-allow`. Detach closed the channel; the next node read returned `NOT_ATTACHED`, exit 4; the sample process remained alive and was then explicitly terminated. No unrelated application was launched, attached, signed, or quit, and no security/grant changes were made. This run was not an attach-to-running, AX-correlation, or comprehensive no-focus test.

**Source-backed value to preserve:** [`AppKitWalker`](https://github.com/markmals/apple-platform-tools/blob/3e587d4f0f7a5c771234fdf52ac409b6d58a65ff/Sources/RuntimeKit/AppKitWalker.swift) obtains native runtime classes, fonts, materials, geometry and constraints on the main thread. [`LayerSnapshot`](https://github.com/markmals/apple-platform-tools/blob/3e587d4f0f7a5c771234fdf52ac409b6d58a65ff/Sources/RuntimeKit/LayerSnapshot.swift) recursively walks actual sublayers; this is implemented, not merely planned. Plain snapshots separate AppKit access from projection and serialization.

**Compromises not to copy:** [`LiveTreeResolver`](https://github.com/markmals/apple-platform-tools/blob/3e587d4f0f7a5c771234fdf52ac409b6d58a65ff/Sources/UIToolServer/LiveTreeResolver.swift) re-resolves ordinal/class paths, which identify a current location rather than proving the same object still occupies it. Boot epochs use whole wall-clock seconds, sockets are predictable per-PID paths without peer authentication, and detach's CLI suppresses transport errors. [`MainThreadHop`](https://github.com/markmals/apple-platform-tools/blob/3e587d4f0f7a5c771234fdf52ac409b6d58a65ff/Sources/UIToolServer/MainThreadHop.swift) bounds the waiting caller but does not cancel an already queued or executing read. The server snapshots a broad forest before client-side projection, and the walker gathers rich fields even when only a small projection is returned. Its hand-written injector is plain-arm64-specific; its research/arm64e readiness is not implemented. These are source observations and design limitations, not runtime claims that the audit exercised each failure.

Construction intent is also not a runtime oracle: the fixture's nominally unbacked `plain` view had a backing layer after display on this OS. Tests must distinguish a deliberately controlled absent layer from assumptions about AppKit's display policy. General font inventory, independent layer handles, explicit cell records, and host-side AX correlation are new Recon work, not an already-complete port.

**Other prior art:** [Flexscope](https://github.com/markmals/flexscope/tree/144d50b82462e1a328123794a21ddcce8d92e811) supplies specifications, not an executable implementation. Its AX comparison and attachment choices are not settled evidence. [FLEX PR #740](https://github.com/FLEXTool/FLEX/pull/740) is a closed, unmerged AppKit-walker effort, not a general runtime-library port. Frida's [C API](https://frida.re/docs/c-api/) is a candidate embedded bootstrap dependency; it was not installed or exercised here. Its [wxWindows library license](https://github.com/frida/frida-core/blob/main/COPYING), binary-linking exception, bundled dependencies, artifact provenance, and footprint must be evaluated before adopting it.

## Proposed solution

Add a `native` command family backed by a Recon-owned, locally loaded inspector. Keep the Rust CLI and existing contract infrastructure; do not require running `uitool`, Flexscope, the FLEX GUI, or an external scripting CLI. The helper implements a closed set of inspection operations, not an arbitrary Objective-C/JavaScript execution service. The native objects and their observation contract remain Recon's even if an embedded, version-pinned library supplies the low-level loader.

Recommend a maintained native injection backend, evaluated through frida-core's embedding API, over porting the prior art's fixed-delay arm64 remote-thread bootstrap. This recommendation is not a settled dependency decision: the research/arm64e proof and dependency review must resolve Open questions first. A cooperative-only fallback cannot discharge the research-machine requirement.

```text
resolve target → report access facts → explicitly open session
             → inspect native structure → correlate separate AX evidence
             → close session, reporting any remaining resident helper code
```

## Detailed design

### 1. Supported targets, access, and command surface

The required host and native target matrix is Apple silicon on macOS 27, including compatible arm64e protected-app targets under the research profile. Intel, Rosetta-translated targets, older macOS, UIKit/Catalyst, Simulator, and physical iOS devices are explicitly unsupported by this proposal. That restriction applies only to the new capability, not existing commands.

Use the existing exact application-name, bundle-identifier, or absolute `.app`-path selector. A running instance is chosen by `--pid` when needed; ambiguity remains an error. A raw executable path or bare PID does not bypass bundle resolution and ownership checks. A symlink alias must resolve to the same selected bundle rather than changing which copy is inspected.

| Command | Behavior |
| --- | --- |
| `recon native preflight TARGET [--pid PID]` | Read target/host facts and report eligibility and unresolved prerequisites for each access mode/profile; never load code, launch, request a grant, or alter security. |
| `recon native open TARGET --mode existing\|launch\|attach [--profile standard\|research] [--pid PID] [--allow-authorization]` | Establish one explicitly requested session; `standard` is the default profile and `--mode` has no default. |
| `recon native close SESSION` | Close and invalidate that session without quitting the target or pretending loaded code was unloaded. |
| `recon native windows SESSION` | Enumerate native windows/panels and child-window/sheet relationships, including invisible windows present in `NSApp.windows`. |
| `recon native tree SESSION [--root HANDLE] [--kind view\|layer] [--depth N] [--fields LIST] [--limit N]` | Return a bounded native subtree or window forest; layer traversal requires a view or layer root. |
| `recon native node SESSION HANDLE [--fields LIST]` | Read one live object and its requested facets; relationships are references, not an implicit recursive expansion. |
| `recon native find SESSION [--root HANDLE] [--kind view\|layer] [--class NAME] [--identifier TEXT] [--depth N] [--fields LIST] [--limit N]` | Search the selected live scope using exact filters, ANDed when combined; at least one filter is required. Layer search requires a view or layer root. |
| `recon native fonts SESSION [--root HANDLE] [--depth N] [--limit N]` | Inventory fonts observed in the selected view/window scope, preserving source objects and coverage. |
| `recon native correlate SESSION WINDOW_HANDLE [--depth N] [--limit N]` | Gather separate native and existing Recon AX observations for one window and report evidence-backed candidate relationships, not object equality. |

`existing` connects only to a compatible Recon helper already present in the selected process; it never loads a new image. `launch` starts a non-running selected app with the helper explicitly enabled. It refuses an already-running target rather than activating it, replacing it, or manufacturing a fresh instance behind the caller's back; `--pid` is invalid with this mode. `attach` loads the helper into a selected running process without terminating or relaunching it. There is no automatic mode fallback and no `--replace` in this proposal.

The standard profile uses existing cooperation, signing, and debugger permissions. The research profile additionally permits an explicit attempt against a non-cooperative target on an already-prepared research host. It never overrides target identity, peer authentication, architecture, or protocol checks. Neither profile disables SIP/AMFI/library validation, changes boot arguments, re-signs a target, installs an elevated service, invokes `sudo`, supplies administrator credentials, or grants TCC access. If debugger authorization can be requested by the selected mechanism, preflight names that possible effect and `open` requires an explicit `--allow-authorization` flag before permitting the request; without it, return an access-prerequisite failure without a dialog. Denial never triggers escalation or retry under another identity.

`--allow-authorization` is accepted only with `--mode attach`; using it with `existing` or `launch` is a usage error. It permits only the selected mechanism's disclosed macOS debugger-authorization interaction, not elevation, security reconfiguration, target signing, or an alternate identity. If the backend cannot guarantee a no-dialog attempt without the flag, refuse before invoking that operation and report the unresolved authorization prerequisite. That no-prompt boundary must be demonstrated when selecting the loader.

Preflight reports bundle/process/architecture facts, helper availability and compatibility, observed signing/entitlement/protection facts, and per-mode `eligible`, `blocked`, or `unknown` with reasons and evidence sources. Static facts establish eligibility, not successful attachment. Inaccessible facts are unknown, not false. It is valid to succeed with a report that every mode is blocked; an unresolvable or ambiguous target still uses the existing targeting error contract. No system-wide “research ready” Boolean substitutes for target-specific evidence.

### 2. Session identity, trust, and lifecycle

A successful open returns the exact bundle path/id, PID plus independently checked process-incarnation identity, OS/build and target architecture, random session identifier, actual access mode/profile, helper build identity, wire protocol version, capabilities, and effects performed. Success requires an authenticated target-side handshake and bounded identity read, not a socket appearing or an injection API reporting success. Verify the selected process again across the open race; PID reuse or a changed executable cannot redirect a session.

Attachment must bind the loader to the verified process incarnation before the first target-side effect, using a stable kernel process/task identity rather than an unchecked numeric PID. Revalidate the selected incarnation and executable against that binding before loading or writing target memory. A post-load handshake is necessary but cannot repair injection into a replacement process. If a loader cannot provide this pre-effect binding, it is not eligible for this contract; repeated PID lookups before and after injection are not an equivalent guarantee. Controlled exit/PID-reuse races must demonstrate refusal without modifying the replacement target.

Use a local-only channel with kernel-backed peer identity checks and an unguessable per-session capability. A filesystem endpoint, if used, lives in an exclusively created owner-only directory with owner-only endpoint/descriptor permissions; do not trust or unconditionally unlink a predictable `/tmp/<pid>` path. Bind the handshake to the expected process incarnation, helper build, session nonce, and owning user. Do not emit capability secrets in command lines, output, or logs. Same-UID processes able to read the owner's files or memory are not promised an adversarial isolation boundary; the protocol still rejects unauthenticated, wrong-process, and cross-session requests. No network listener or arbitrary-code endpoint is introduced.

Only matching supported protocol/helper builds may communicate. A mismatch fails explicitly; it does not silently replace resident code, downgrade behavior, or activate an older compatibility path. `existing` or `attach` may reuse a still-valid session for the same process, profile, and build, returning `reused: true` without injecting again. Return the new request's mode separately from the mode that originally established the session; connecting with `existing` does not rewrite a prior `launch` into a different acquisition history. A profile/build conflict fails rather than upgrading the session. `launch` always refuses a running target, even if it already has a session. A process may have successive sessions, but only one active Recon session at a time in this proposal.

Helper residency and session lifetime are separate. A resident helper keeps an authenticated, local-only bootstrap endpoint and an owner-private discovery descriptor until process exit, even when no inspection session is active. Bootstrap permits only identity/compatibility negotiation, session-state reconciliation, and session establishment/revocation; it cannot read native objects. `open existing` verifies the descriptor's ownership, live process incarnation, and kernel-backed peer identity before establishing a new random session capability and empty handle namespace. Session close revokes every previous capability and handle without destroying this bootstrap path. One transport may serve both roles, but an inactive helper rejects every native operation until a newly authenticated open; stale descriptors or capabilities cannot reopen a session automatically.

Native handles are opaque session-scoped identities for windows, views, layers, cells, and referenced layout guides. A structural path is optional descriptive location, never the identity used for later reads. Maintain weak object identity without retaining the inspected graph for the life of the session; temporarily retain a resolved live object for the bounded read. Reordering, reparenting, or property changes preserve a surviving object's handle. Deallocation or replacement makes its old handle stale, even if the address, class, frame, or child index is reused. Closing/reopening a session, process exit, or relaunch invalidates every old handle. Never resolve a stale handle by searching for a similar replacement.

The registry has a 100,000-live-handle ceiling; remove dead weak entries, never recycle identifiers within a session, and report registry exhaustion as unavailable capacity rather than evicting a live object into a silently different identity. Historical observations remain readable JSON after handles expire; this does not implement #23's evidence store.

Close revokes the session capability, ends its inspection channel, drops registry state, and removes only its session-owned endpoint/descriptor, if separate from bootstrap. It leaves the target running and reports the resident helper and bootstrap endpoint separately from the closed session. No safe `dlclose` is promised for a library that registered Objective-C classes or otherwise joined the runtime. Acknowledged close and an already-closed locally known session are distinguishable successful results. A missing acknowledgment or transport failure is an unknown teardown outcome, not invented success. Process exit invalidates both bootstrap and session access; a later command can remove a stale local descriptor only after verifying its ownership and incarnation. There is no automatic reattachment.

An unacknowledged close enters `close-unconfirmed` locally. Retain the owner-private recovery descriptor and exact session/incarnation identity; never discard it and inject another helper. Refuse native reads and session reuse while revocation is uncertain. A repeated `close SESSION`, or an explicit `open` for that same process, first reconciles the named session through authenticated bootstrap state. Confirmed revocation or confirmed process exit resolves the uncertainty. If the old session remains active, repeat close explicitly; `open` returns `INSPECTION_BUSY` with that recovery action rather than reusing it. Unreachable bootstrap preserves the unknown outcome and refuses a replacement session. Reconciliation must not evaluate target UI properties or equate a missing local file with revocation.

### 3. Native structure and field semantics

Window enumeration includes native class, title/identifier when available, frame, visibility, key/main facts, content-view reference, child-window and sheet relationships, and the native window number where assigned. A missing number is not replaced by an ordinal. Enumerate each window once, retaining observed relationship/order information; a titleless, hidden, off-Space, or child window is not discarded merely for being inconvenient to correlate with WindowServer.

A view traversal begins at the requested view, the content hierarchy of a requested window, or each window's content hierarchy when no root is supplied. Each window record also exposes the topmost view reachable through that content view's same-window superview chain, when distinct, so an explicit root query can inspect AppKit's frame/chrome realization. Do not force a missing content view, load a controller's uninstantiated view, or create windows to fill the result. Preserve `subviews` order and real runtime class/superclass names, including private and KVO subclasses. Model `NSControl.cell` as a related cell, not a subview; layout guides and constraints are also relationships rather than fabricated visual children.

Layer traversal reads model-layer containment independently through `sublayers`. A view root selects its existing associated layer; no associated layer is a known absent relationship and a successful empty layer result. Never set `wantsLayer`, call `makeBackingLayer`, or force drawing/layout to obtain more data. Preserve layer identity when a layer is referenced from both a view and a layer relationship; a mask is a distinct relationship, not an invented sublayer. Report repeated references and cycles without recursively expanding them. Presentation-layer state and animation tracing are not included; results explicitly say `layerState: model`.

The default field set is `basic`. `--fields` is a comma-separated, non-empty set of the following groups; unknown or repeated groups are usage errors. Identity, kind, relationships needed to interpret the returned structure, and completeness metadata are always present. No group is fetched merely to discard it later.

| Group | Required observations where applicable |
| --- | --- |
| `basic` | Runtime class/superclasses, identifier, frame/bounds, coordinate metadata, own hidden/alpha state, and child counts. Visibility is observed state, not a claim that the object contributes visible pixels. |
| `appearance` | Effective appearance, supported control/text/background colors, and `NSVisualEffectView` material, blending mode, state, and emphasis. Retain named/catalog color identity when present alongside an explicitly identified resolved color space/components; unsupported conversions are visible. |
| `font` | Font carrier/source, family, PostScript name, point size, raw descriptor traits and weight; control/cell fonts and bounded attributed-text font runs where available. Do not invent a system font when no font was read, or present a nearest weight name as the raw trait. |
| `layout` | Constraints held by the object and touching it through its ancestor chain, deduplicated by constraint identity; owner, first/second item kinds and references, attributes, relation, multiplier, constant, priority, active state, and identifier. Include intrinsic size and hugging/compression resistance by axis. Preserve the no-intrinsic-metric sentinel distinctly from a measured size. This is not a complete solver explanation. |
| `layer` | Existing associated layer reference; model geometry, transform, contents scale, hidden/opacity, corner/masked-corner, border, shadow, clipping, and color facts. It does not recursively fetch the layer tree; use `--kind layer` for that. |
| `cell` | Existing cell reference and native class, supported cell type/state, enabled/editable facts, and font source. Cells are not enumerated as view children. |

A field known absent, unsupported for that object, denied, failed, or non-finite has an explicit per-field status. Unrequested fields are absent without a status. Preserve legitimate zero, false, and empty values. The response schema defines each group's typed values and status vocabulary; errors do not masquerade as empty values. Typed allowlisted AppKit access replaces arbitrary selectors, KVC paths, `description`, property enumeration, or generalized invocation. This still executes possible overrides; command effects must disclose that fact. Exceptions can become field failures only where a safe exception boundary exists; a crashed target is a failed observation, never a partial success assembled from guessed values.

Frame and bounds carry their coordinate space and units. View frame means its superview's logical points, with that coordinate system's orientation recorded separately from the view's own `isFlipped`. Window frames use AppKit global-screen logical points. Include a window-relative and global-screen normalization only when it can actually be computed, together with the display/scale transformation used. Do not multiply all values by a presumed Retina scale or equate model-layer geometry with current on-screen geometry.

Mark recognizable SwiftUI hosting boundaries with the evidence used to recognize them and `swiftUIContent: not-inspected`. Runtime names and native descendants remain useful evidence, but a substring match alone is a heuristic, not proof of a recovered SwiftUI view type. No AttributeGraph walk or declarative hierarchy is implied.

The font inventory groups identical observed font descriptors and lists contributing object handles and source kinds, including bounded attributed-run ranges without exporting text content. Traversal coverage and run limits accompany the inventory; a partial walk produces a partial inventory, not an exhaustive list of fonts rendered by the application. General text extraction, image bytes, and layer contents are not default outputs of this feature.

### 4. Bounded execution and observation results

Every operation validates the session and request before AppKit work. Traversal and object-specific reads run on the target main thread; host-side decoding/projection/serialization uses copied observation values, not live pointers. The helper receives the requested root, filters, field groups, and bounds. It must not inspect an unrelated window forest or every rich field merely because the CLI will later filter it.

Tree/find/font traversal defaults to depth 3 and 200 returned records. Depth ranges from 0 through 64; limit ranges from 1 through 25,000. A selected root is depth zero; in the default view forest each window content root is depth zero and window metadata is outside view depth. Layer depth is relative to its selected layer root. The hard work ceiling is 25,000 visited objects/relationships per request across windows, views, layers, constraints, and attributed runs, with at most 256 runs examined per text carrier. Window nesting consumes the same request budget. Requested limits and hard limits both constrain target work, not just printed rows. A find limit stops after enough matches, with the unsearched scope identified; it never reports a full match count it did not compute.

Cap each request at 64 KiB and each complete response at 8 MiB; cap each returned string at 4,096 Unicode scalars and mark any truncation. Enforce framing/allocation limits before trusting lengths supplied by either endpoint. The same bounds apply to nested layers and field arrays; wide or cyclic graphs cannot bypass them. A valid smaller result identifies exactly which requested scope was narrowed and which reads were incomplete, using separate `narrowed` and `truncated` reasons rather than one ambiguous flag.

Use a two-second read deadline covering queueing, target execution, transport, and response delivery, and a ten-second open/close deadline. A queued read whose deadline expired must not begin later. Check the budget between individual reads; never kill the target or asynchronously unwind its main thread to enforce it. A getter already executing when the deadline expires may continue or remain blocked. Return a timeout with its stage and `targetExecutionMayContinue: true` when appropriate; do not claim rollback, cancellation of target code, or a coherent result. Allow only one outstanding UI read per process incarnation, not merely per session. After an unresolved execution timeout, refuse new UI reads until that work actually finishes or the process exits. Close may revoke access, but closing/reopening does not clear the in-flight guard or imply target execution stopped. A new session reports the process as busy until the old work completes; its old response cannot become a result for the new session. Lifecycle/transport handling remains separately responsive where possible.

Successful observations include application/process incarnation, session/helper/protocol identity, source `native-appkit`, requested scope/fields, unique observation id, start/end times, OS/build/architecture, and explicit coverage. The consistency claim is main-thread, best-effort observation over that interval—not an atomic snapshot of the whole application or compositor. An empty fully observed scope is distinguishable from an unavailable target. Preserve one JSON object on stdout and the existing typed error envelope on stderr; no helper logs, JSON-Lines footer, or raw target output contaminate that contract.

### 5. AX correlation and explicit effects

`native correlate` captures native and AX evidence separately for the same verified process incarnation and selected native window. Map to the existing UI window contract only when a real window number and process match are available; otherwise return an explicit unavailable correlation rather than guessing by title. Record independent observation ids/times, coordinate transformations, and completeness for both sides. Native object handles and AX snapshot paths remain different identifiers.

Candidate matches use a non-empty exact accessibility identifier observed on both interfaces where available; otherwise they require matching self-reported AX role and normalized screen bounds within one logical point on every edge. Native `NSView.identifier` is not silently treated as `accessibilityIdentifier`. Retain every candidate when ambiguous. Record the matching basis; even a unique candidate is not asserted to be the same underlying object. Unmatched nodes are a difference between evidence sources, not an automatic accessibility defect or an instruction to synthesize a node. In-process accessibility getters are marked as target-executed reads, not substitutes for the external AX capture.

External AX acquisition reuses Recon's existing privacy identity and permission behavior under decision.0002. Failure to obtain that required observation fails the correlation command with its typed cause; it does not alter or disable the independent native inspection commands. An explicit AX correlation request may follow the existing permission-request policy; preflight never does. No automatic screenshots, mouse movement, AX actions, focus change, layout flush, target relaunch, or UI mutation is part of a structural query.

Registry effects distinguish `loads-target-code`, `launches-application`, `may-request-debugger-authorization`, `executes-target-getters`, and local session-file writes where applicable. Preflight reports facts without these activation effects. Launch and arbitrary target overrides can have application-owned side effects; the contract promises which actions Recon itself does not perform, not that executing target code is universally side-effect-free. Existing `ui` no-focus behavior stays intact and is verified independently.

### 6. Failures and required verification

Reuse existing selection/usage/host/privacy errors where their meanings still apply. Add closed, schema-published native classifications rather than repurposing the current AX-specific `TIMEOUT`: `INSPECTION_UNAVAILABLE` (including unsupported target ABI/build or missing access prerequisites), `INSPECTION_PROTOCOL_MISMATCH`, `INSPECTION_FAILED`, `INSPECTION_TIMEOUT`, `INSPECTION_BUSY`, `STALE_SESSION`, and `STALE_NODE`. Native failures identify the stage, target/session where known, observed cause, and whether target execution or helper residency may remain. Malformed handles/options are `INVALID_ARGUMENT`; a well-formed expired identity is stale. Do not mark an uncertain execution timeout or failed attach as automatically retryable. Read or close failures never become a successful empty tree or a fictitious teardown acknowledgment.

Implementation evidence must cover these six coherent outcomes:

1. **Access:** A real helper handshake and native read through Recon for cooperative launch, an already-present helper, and permitted live attach. On an operator-authorized, already-configured research Mac, also prove a same-user non-cooperative third-party target and a protected Apple AppKit target, including an arm64e case. Record exact target/OS/helper/loader/signing/security conditions. A denied attempt on a stock Mac is not the research success case.
2. **Lifetime and trust:** Exit/relaunch/PID reuse, two installed app copies, peer/process/protocol mismatch, close acknowledgment loss, same-session sibling reorder/replacement, deallocation/address reuse, and reconnect never alias a new target or object. Verify pre-effect loader binding prevents injection into a replacement process. Close leaves the target alive and reports residual code honestly; lost acknowledgments retain recoverable uncertainty until authenticated reconciliation.
3. **Structure and facets:** Controlled fixtures provide native-only views, a custom/private subclass, a cell, child windows/sheets, independent sublayers and masks, controlled absent layers, named/dynamic colors, fonts and attributed runs, ancestor-owned constraints, layout guides, and SwiftUI hosting. Oracle-owned values are exact; OS-private class spelling and incidental layer creation are observations, not pinned implementation tests.
4. **Bounds and effects:** Exercise zero depth, projection that does not evaluate excluded getters, repeated/cyclic relationships, wide graphs, oversized frames, malformed input, busy main thread, an expired queued request, and a deliberately blocking/throwing getter. No delayed queued read executes after cancellation; an already-running blocked getter is not falsely claimed stopped, and closing/reopening cannot bypass its process-wide in-flight guard. Verify foreground/input/target state under controlled conditions rather than attributing unrelated user activity to Recon.
5. **Correlation:** Compare actual external AX with native evidence for duplicate identifiers, omitted native-only nodes, ambiguous bounds, differing observation times, flipped views, non-unit scale, and negative/multiple-display coordinates. Missing AX permission must not prevent independent native queries.
6. **Delivery:** Tests derive from this design before guides and implementation. Validate real outputs against the registry schemas, run `vp run check`, exercise signed permission-gated regression checks, obtain independent security/ontology and adversarial review, and provide an installable private preview with transcripts of both access profiles. Missing research evidence or disputed identity/security findings block acceptance of #51 and #44; they cannot be filed away as follow-up work.

## Compatibility

The new family is additive. `app`, `ui`, `schema`, and `context` retain their existing meanings and privacy identity; `ui tree` remains AX-only and never silently upgrades to injection. New error codes and command effects are discoverable through the existing registry. There is no stored-data migration, compatibility alias, public wire-compatibility promise for a mismatched helper, or dependency on #23's unfinished persistence service.

The proposal does not require every native consumer to wait for this view inspector: #51's access/identity/lifecycle outcome remains separately verifiable for #13/#45/#48/#50. This proposal does not implement those consumers, or Simulator/device sessions #52/#53.

## Implications on adoption

Users install a matching CLI/helper build and opt into an explicit native session. The final loader choice may add packaged native dependencies and licensing notices; this must be settled before implementation approval, not downloaded dynamically during inspection. The preview must identify its helper/loader artifacts and verified architectures.

Cooperative targets retain their own signing/cooperation requirements; Recon does not alter another app to make it eligible. Research users supply and authorize an already-prepared macOS 27 machine and accept the target-execution risk. No new hardware, service, certificate, or paid resource is authorized by this proposal. Closing a session revokes inspection access but may leave code mapped until the app exits; callers decide whether and when to restart their application.

## Scope

- The #51 macOS session outcome and the #44 native view/layer outcome, delivered together with real cooperative and research-machine evidence.
- Target-specific preflight, explicit modes/profiles, authenticated local communication, process/session/object lifetime, and truthful cleanup.
- Native windows/views/cells/layers, supported visual/layout/font facets and inventory, bounded search and drill-down, SwiftUI boundaries, and separate AX correlation.
- Registry/schema/errors, controlled fixtures, user guides, private preview, and the verification above.

### Out of scope

- Automatically weakening protections, changing another app's signature/entitlements, installing an elevated daemon, acquiring credentials, or claiming access to every protected target.
- Intel, Rosetta targets, older macOS, UIKit/Catalyst, Simulator, physical iOS devices, Windows, and Android; none is silently counted as covered by an Apple-silicon Mac test.
- General class/heap/ivar exploration (#45), arbitrary method invocation or state edits (#49), network hooks (#48), resource bytes (#13), application-local state (#50), and UI actions (#24).
- SwiftUI value/AttributeGraph reconstruction, presentation-layer/animation tracing, a complete Auto Layout solver explanation, live event streaming, remote control, a plugin SDK, or an external scripting shell.

## Preview

- Repository shape/artifact: A locally installable macOS 27 Apple-silicon Recon executable, matching helper and any selected loader dependencies, plus a controlled AppKit fixture. The fixture and CLI must be real binaries, not canned JSON or screenshots.
- Private access: A locally built preview from the PR checkout using the existing signing workflow for Recon's own identity, with an explicit artifact directory and build hashes recorded in the readiness report. Research verification occurs on the separately authorized prepared machine; its transcript identifies that environment without publishing sensitive app data. No public release or deployment is authorized.
- Exercise: Preflight, explicitly open each supported access mode/profile, locate a native-only node and related layer, inspect font/layout data, correlate one window with AX, then close and verify stale handles and a still-running target.
- Cleanup: End only sessions and fixture processes created for the preview; remove their owned temporary endpoints and artifacts. Do not reset grants, re-sign user apps, undo someone else's research-machine configuration, or delete branches without authority.

## Policies and decisions checked

- **VISION.md:** The current main working copy records Mark's broader reverse-engineering direction and links #44/#51, but those alignment edits were uncommitted when this isolated branch was created. The committed base still says REA owns native analysis and emphasizes externally observable behavior. This is an explicit record conflict, resolved in product direction by Mark's 2026-10-27 instructions but not yet reconciled in Git history. Integrate the authorized vision alignment before implementation approval; this proposal does not silently rewrite the independent documentation changes or claim the old boundary permits the new native engine. The bounded behavioral MVP remains a separate milestone.
- [policy.0001](../policies/0001-cli-command-and-error-contract.md): Every new operation/effect/error belongs to the existing registry and schema contract, with real output validation. Prior-art command names, JSON-Lines framing, and exit codes are not imported as a parallel convention.
- [decision.0002](../decisions/0002-macos-context-privacy-identity.md): Recon keeps its own macOS privacy identity; preflight does not request permissions; external AX uses the existing permission path. A target-loaded helper does not borrow Recon's TCC grants, and Recon does not use the helper to bypass missing host-side AX permission.
- [decision.0001](../decisions/0001-macos-privacy-identity.md): Read as historical context; it is superseded by decision.0002, not a second current permission rule.
- No new policy or decision record is created while the architecture is draft. The final loader/trust boundary may warrant an architectural decision after human approval; no speculative rule is established now.

## Future directions

The separately tracked runtime, network, asset, and application-state inspectors may reuse the session contract without sharing this view walk. Simulator/device work may reuse concepts only after its own access and lifecycle evidence. Broader OS/architecture coverage, animation state, and deeper SwiftUI research are independent changes. None replaces the research-machine access already required here.

## Alternatives considered

| Approach | Tradeoff and disposition |
| --- | --- |
| Cooperative embedding/launch only | Smallest access mechanism and useful for development apps, but excludes the required research-machine outcome. Mark explicitly chose the broader boundary. |
| Port the prior art's remote injector and expand it for arm64e | Maximizes local ownership and avoids a substantial dependency, but inherits fixed-delay thread/bootstrap assumptions and makes Recon responsible for PAC, dyld, code-signing, and OS-version regressions. Not the recommended default without stronger evidence. |
| Recon-owned inspector with a maintained embedded injection library | Recommended for evaluation: retain Recon's native command/data/session behavior while reusing difficult loader engineering. Adds artifact, license, build, and trust review; published Frida support is not proof on macOS 27. Selection is still open. |
| External `uitool`/Frida CLI wrapper or full FLEX application port | Fast access to some existing behavior, but a second targeting/error system, GUI dependency, or arbitrary scripting contract would replace rather than implement Recon's native capability. Rejected. |
| AX enrichment or out-of-process memory scraping alone | AX cannot establish the native graph it deliberately omits. Raw memory reconstruction must solve runtime/layout/lifetime issues and does not safely replace main-thread AppKit evaluation. Neither satisfies the specified outcome. |

## Open questions

- [NEEDS CLARIFICATION: Which pinned native loader and helper architecture will provide permitted attach and the required macOS 27 research/arm64e path? Evaluate the embedded frida-core recommendation against a Recon-owned injector, record artifact/license/dependency and cleanup consequences, and obtain controlled attachment evidence before treating the architecture as settled. Published support and the successful DYLD fixture run are not that evidence.]
- [NEEDS CLARIFICATION: Which operator-authorized, already-configured macOS 27 research machine and exact non-cooperative third-party/protected Apple targets define the required success matrix, and which target-scoped attachment operations may be exercised there? The current machine has SIP enabled; no permission to reconfigure it, elevate, or attach to unrelated applications has been granted. This is a validation prerequisite, not permission to reduce the accepted scope.]

## Acknowledgments

apple-platform-tools, Flexscope, and FLEX inform the inspection behavior; Apple's documentation establishes the native-object and protection boundaries. Frida supplies relevant injection prior art to evaluate. Sources and published support are credited without treating them as executed compatibility evidence.
