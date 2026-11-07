---
id: proposal.0007
title: macOS UI Interaction
authors: [Mark Malstrom, OpenAI Codex]
status: draft
pull-request: https://github.com/markmals/recon/pull/91
issues: ["https://github.com/markmals/recon/issues/24"]
supersedes: []
---

# macOS UI Interaction

## Summary

Add clicks, scrolling, drag within and between applications, hover, and window-targeted key chords without disturbing the user's focus, pointer, Spaces, or input. One click interface selects AX or mouse delivery before acting and reports the mechanism. AppKit and Mac Catalyst require independent actuation evidence, including selected-window keyboard delivery.

## Motivation

Recon can inspect an application's interface but cannot perform the next step in an investigation: press an observed control or invoke a shortcut and observe what changes. An agent should be able to investigate a background application while the person using the Mac continues working elsewhere. Bringing the target forward, operating it, and restoring the desktop does not satisfy that goal: the interruption itself is the failure.

Issue [#24](https://github.com/markmals/recon/issues/24) identifies winctl as prior art and separates explicit actions from the completed observation work in #15. Its example commands express the desired workflow, not an approved grammar. Native in-process inspection is not a prerequisite for an external action. Mac Catalyst is a required target family, not an inference from an AppKit demonstration.

The selected scope is a unified click interface with automatic mechanism selection and an explicit override, left/right/middle and double/modifier clicks, horizontal/vertical scrolling, drag within a window and between independently selected windows of the same or different applications, hover, and key chords. Selected-window keyboard delivery, including secondary/non-key windows, remains required rather than a process-directed substitute. Trackpad pinch/zoom, rotation, and swipe are preserved separately in [#92](https://github.com/markmals/recon/issues/92). This draft does not claim that the required external mechanisms have already been proven.

## Domain grounding

### Established context

- Apple's [AXUIElementPerformAction](https://developer.apple.com/documentation/applicationservices/1462091-axuielementperformaction) requests an action on an accessibility object. The installed Xcode 27.1 `HIServices/AXUIElement.h` contract, lines 314–331, explicitly says `kAXErrorCannotComplete` can occur when an action callback performs modal processing and does not return before the messaging timeout; it does not necessarily mean the action failed. Retrying a mutating operation can therefore duplicate its effects.
- Apple's [CGEvent.postToPid](<https://developer.apple.com/documentation/coregraphics/cgevent/posttopid(_:)>) takes a PID, not a window identity, and returns no delivery acknowledgment. The installed `CoreGraphics/CGEvent.h`, lines 356–374, describes application event-stream routing. This establishes neither selected-window delivery nor application-level actuation.
- The same header separates non-prompting `CGPreflightPostEventAccess` from potentially prompting `CGRequestPostEventAccess`, and likewise distinguishes event-listening preflight from its request. An Accessibility grant or a successful post on a weakened research host is not sufficient evidence for the normal event-posting permission contract.
- [winctl's pinned hit-testing experiment](https://github.com/NSExceptional/winctl/blob/295a02751b2cb0c958b476e6ce6864963ee4042f/research/hittest/README.md) records a background AppKit button actually firing on macOS 15.7, Apple silicon, SIP disabled and AMFI relaxed. A bare mouse post reached the process with window number zero but did not actuate the button. Setting private routing field 51 plus mouse fields made it actuate. The experiment does not establish Catalyst behavior or behavior on macOS 27.
- That experiment distinguishes an unlocked session from a password-locked screen. Its synthetic input stopped arriving after lock. It also distinguishes focus-without-raise from genuinely background delivery. Neither a screensaver demonstration nor an unmoved cursor proves preservation of keyboard focus.
- [winctl's implementation at the same revision](https://github.com/NSExceptional/winctl/blob/295a02751b2cb0c958b476e6ce6864963ee4042f/winctl.m) uses PID-only posting for `key` even after selecting a window. Its mouse path posts each event through both `SLEventPostToPid`, when available, and `CGEventPostToPid`. The former is not evidence of selected-window keyboard delivery; the latter must not be copied as a mechanism that might double-actuate.
- The [upstream cua keyboard implementation](https://github.com/trycua/cua/blob/1759af24ec84685269529425ac42e5de964adf85/libs/cua-driver/rust/crates/platform-macos/src/input/keyboard.rs), inspected during this draft with blob identity `1553fda86e6c1bae13dbf880b5786a7ad37bf7ca`, exposes PID-directed key operations. It separately describes global-HID helpers as requiring foreground activation. Those helpers are not a compliant fallback for Recon. These are source observations, not independent execution evidence.
- Apple's [CGEvent API](https://developer.apple.com/documentation/coregraphics/cgevent) provides mouse and scroll-wheel event constructors. The installed Xcode 27.1 `CoreGraphics/CGEvent.h`, lines 43–60 and 84–109, distinguishes mouse event type/button from line- or pixel-based scroll deltas. `CGEventTypes.h` also distinguishes click count, scroll phase, and momentum phase. Being able to construct an event does not establish selected-window routing or gesture recognition.
- Apple's [NSEvent.EventType](https://developer.apple.com/documentation/appkit/nsevent/eventtype) distinguishes right/middle button events, mouse motion/dragging, and scroll-wheel input from touch-based magnify, rotate, and swipe events. A right-click is not an AX press; a wheel scroll is not a trackpad swipe or pinch. An application may interpret modifier flags or gesture phases differently, so event fields and resulting behavior both need evidence.
- Apple's [NSDraggingSession](https://developer.apple.com/documentation/appkit/nsdraggingsession) represents a drag-and-drop action, including its dragging pasteboard and lifecycle; beginning it returns before the drag actually starts. [NSDraggingDestination](https://developer.apple.com/documentation/appkit/nsdraggingdestination) separately defines destination entry/update/exit, acceptance or refusal, and importing dragged data. Posting a down/up pair to different windows does not establish this protocol, accepted data transfer, or its no-interference properties.

### Relevant constraints and principles

An application bundle, a process incarnation, a WindowServer window, an AX element, and a native view are different things. A bundle can have several running instances; a process can own several windows; an AX label or identifier is not necessarily unique. A historical tree path is an observation location, not a durable action handle. The same numeric PID can name a different incarnation after exit/reuse, and this project's recorded research also observes an incarnation change across `exec` without a PID change.

A drag has source and destination endpoints, each bound to its own process incarnation, window, and local coordinates. They may share a process or even a window, but neither determines the other's identity. The drag session, its offered data/operations, destination acceptance, and source completion are distinct from pointer event delivery; they do not create a new native inspection session or permission to inject a helper.

AX press is a semantic action supplied by an application; a mouse click is a button gesture at a point. They can satisfy the same plain element-click intent, so they share one command, but remain distinguishable mechanisms in its explicit override and result. Right/middle clicks, double-clicks, and modifier-clicks require mouse semantics rather than repeated AX calls. Scrolling, drag, and hover likewise have different event sequences and effects. A key chord depends on the receiving window, responder, menu handling, and keyboard mapping. Selecting a window, delivering an event, invoking a callback, and achieving the caller's application-level goal remain separate facts.

Target-internal responder changes can be intended consequences of an explicit action. Changing which application or window receives the person's real input is not. Application callbacks can themselves activate UI or keep executing after the requester times out; an external client cannot make arbitrary target code pure or roll it back.

### Quality bar

For every supported action, evidence must show the intended effect in the selected window or drag endpoints and preservation of the host user's input and focus throughout the operation. Cross-window dragging additionally needs destination acceptance and data-transfer evidence, not just events received by each endpoint. Two unchanged endpoint snapshots, a successful API return, or an AppKit run labeled Catalyst are insufficient. Unsupported scope must be rejected before input, not ignored. Unknown execution after a timeout must remain unknown, without automatic retry.

### Remaining uncertainty

There is no new action-execution evidence from this proposal yet. On the research host, `sw_vers` reported macOS 27.0.1, build 26A434, and `xcodebuild -version` reported Xcode 27.1, build 27A9269. `mise run dev -- context` succeeded without prompting and reported that this worktree's executable was responsible for itself but lacked both Accessibility and Screen Recording grants. No permission request, security modification, or UI action was performed as part of that context check.

The separately installed Homebrew Recon 0.1.5 also returned a non-prompting context report, with Accessibility and Screen Recording granted to its installed executable. Those grants do not transfer to this worktree or to a new research controller, and that released executable does not implement these actions. The installed binary was not replaced or re-signed.

The required selected-window keyboard mechanism, external routing for each pointer interaction, stock-host permission requirements, incarnation-binding limits, and the event-trace design remain research questions. Cross-window/application drag requires a real source-to-destination drag session and data exchange without capturing host input; same-window dragging cannot establish it. Context-menu tracking and hover behavior also need evidence independent of real-cursor movement. Symbol availability and the existing left-click experiment cannot resolve these questions. The support matrix below is a required outcome, not a report of current support. The draft stays unapprovable until the Open questions are resolved with evidence and concrete design.

## Existing baseline

Recon already supplies strict bundle/path/name selection, instance disambiguation, window ownership checks, optional session identity assertions, executable-owned privacy identity, typed errors, and registry-derived help and schemas. Preserve these contracts rather than adopting winctl's substring/largest-window selection. Relevant seams are `recon-applications-macos/src/selection.rs`, `recon-window-server-macos/src/window.rs`, and `recon/src/observe/target.rs` under `crates/`.

The external AX crate already reads role, title, identifier, enabled state, frame, and advertised actions. `ui ax tree` does not provide durable AX action handles, and the correlation path is not an executable identity. The native session's incarnation checks are useful precedent, but external selection currently returns a PID without binding an incarnation. Reusing an identity concept must not introduce a session or injection requirement.

`crates/recon/src/observe.rs` wraps application-directed UI work in the shared guard. `crates/recon-ui-guard-macos/src/focus.rs` samples roughly every ten milliseconds. Its window-order measurement is not an observation of actual key-window state; foreground, window-order, and Space snapshots can miss changes entirely between samples. Its hardware attribution treats an interval containing hardware input as concurrent user activity. That cannot, by itself, prove that an action caused no simultaneous disturbance. Whether the proposed process-routed events increment its synthetic-input counters has not been measured. These are action-verification constraints, not reasons to disable the guard.

The existing `ReconFixture` is AppKit. There is no Catalyst fixture or live action task in this baseline. The new work must provide those missing proofs rather than relabel the observation tests. Existing native inspection's exclusion of Catalyst does not exclude Catalyst from these external actions or make #55 a whole-feature prerequisite.

## Proposed solution

Expose intent-oriented operations under the existing shared UI namespace, with one click command rather than separate AX and mouse commands:

```sh
recon ui click "Logic Pro" --window 123 --role AXButton --title Bounce
recon ui click "Logic Pro" --window 123 --role AXButton --title Bounce --method mouse
recon ui click "Logic Pro" --window 123 --x 160 --y 84 --button right
recon ui click "Logic Pro" --window 123 --x 160 --y 84 --count 2 --modifiers shift
recon ui scroll "Logic Pro" --window 123 --x 160 --y 84 --dy 300 --unit pixel
recon ui drag "Source App" --window 123 --from-x 40 --from-y 80 --to-target "Destination App" --to-window 456 --to-x 220 --to-y 80 --duration-ms 300
recon ui hover "Logic Pro" --window 123 --x 160 --y 84
recon ui key "Logic Pro" "cmd+b" --window 123
```

These are proposed spellings, not shipped commands or evidence that Logic Pro supports any particular interaction. Plain element clicks prefer supported AX press; an explicit method override or mouse-specific intent determines otherwise. All operations share selection and failure conventions. Automatic selection happens before dispatch and is disclosed in the result; it is not permission to try another mechanism after an action may have started.

Prefer external APIs and runtime-resolved private interfaces where independently proven. A missing capability is an explicit failure. An injected helper, an AX focus write, global HID posting, focus-without-raise, activation followed by restoration, and cursor restoration are not alternatives that complete this proposal. A process-only keyboard command is not a substitute for its selected-window requirement.

## Detailed design

### 1. Command surface and target identity

Every operation selects already-running applications through the existing target grammar. For single-target actions, `--pid` disambiguates the target's instances and `--window` must name a window belonging to that instance. Drag uses those arguments for its source and independently resolves a required `--to-target`, optional `--to-pid`, and required `--to-window` for its destination. The two endpoints may name the same window, different windows of one instance, separate instances, or different applications. Neither endpoint infers the largest, frontmost, main, or key window, and no command launches an application. Existing ambiguity, missing-target, not-running, and wrong-owner failures remain failures with no input.

An optional `--session` is only an additional identity assertion for the single target or drag source, following the existing shared UI contract. Drag accepts an independent optional `--to-session` assertion for its destination; the source assertion is never applied to a different destination implicitly. Without either assertion, supported actions must still work externally. A stale or mismatched supplied session fails; it is not ignored, reopened, or attached as a fallback. Reading an assertion must not dispatch an injected action.

Bind every selected process incarnation and window before resolving the action. For drag, resolve and validate both endpoints and all prerequisites before the initial mouse-down; an invalid destination must not leave a source gesture started. Revalidate bound participants before dispatch, destination handoff/drop, and any later cleanup. An observed exit, `exec`, owner mismatch, replaced selection, or disappeared window stops further interaction on that identity. Do not resolve replacement endpoints and continue. A historical AX path, native node handle, or correlation candidate is not accepted as an action target.

A PID check immediately before a PID-only post is not an atomic incarnation-bound dispatch. The approved design must state the actual binding mechanism and residual race boundary instead of claiming that repeated checks eliminate it. This is part of the unresolved mechanism research, not permission to omit identity checks.

### 2. Unified click and pointer interactions

#### Shared locations and element resolution

`ui click`, `ui scroll`, and `ui hover` accept either one or more of `--identifier`, `--role`, and `--title`, or a complete `--x`/`--y` pair, never both. Supplied AX filters combine with AND and match the corresponding published fields exactly and case-sensitively. There is no fuzzy title search, title/description fallback, or first-match behavior. Roles use AX strings such as `AXButton`; the action namespace does not introduce a second role vocabulary.

Resolve a fresh AX subtree rooted at the selected window. The bounded search must establish one matching element: zero matches is not found, two or more is ambiguous, and an incomplete or timed-out traversal cannot establish uniqueness even if one match was seen. Reuse the existing bounded-read discipline, but never treat omitted/error subtrees as proof that no second match exists. Keep the live element only for this request and revalidate its filters and selected-window membership immediately before dispatch. Historical paths and native handles do not become executable selectors.

Coordinate input is finite logical points relative to the top-left of the WindowServer window bounds, including the frame, not screenshot pixels or AppKit's bottom-left content coordinates. Require `0 <= x < width` and `0 <= y < height`; do not clamp or move the target. Use fresh bounds for conversion and preserve the selected window. Pointer interactions addressed by element use a finite, nonempty frame whose center is inside that window. A semantic AX click does not need geometry.

Element clicks require a known enabled state; disabled or unknown state is unavailable rather than permission to guess. Scroll and hover use the selected element as a location, not as an AX action recipient, so a container need not advertise `AXPress` or a click-specific enabled attribute. Unverifiable membership or required geometry still fails. A covering window must never become the recipient.

#### One click command

`ui click` accepts `--method auto|ax|mouse` with default `auto`, `--button left|right|middle` with default `left`, `--count 1|2` with default `1`, and optional `--modifiers`. Modifiers are a comma-separated subset of `cmd`, `ctrl`, `alt`, and `shift`; reject duplicates and unknown names. Explicit gesture choices are requirements, not hints that an AX backend can discard.

Choose the method before any mutating call:

| Request | Method and behavior |
| --- | --- |
| `auto`, element selector, left button, count 1, no modifiers | Prefer AX only when the element advertises `AXPress` and that action/scope satisfies the evidenced support rules. If it definitively lacks supported AX press, use a supported mouse route at the element center. |
| `auto` with coordinates, right/middle button, count 2, or modifiers | Require mouse delivery. Do not translate the gesture into an AX action. |
| `ax` | Require an element selector, left button, count 1, no modifiers, and supported `AXPress`. Reject contradictory options as `INVALID_ARGUMENT`; unavailable AX support is `ACTION_UNAVAILABLE`, with no mouse fallback. |
| `mouse` | Require mouse delivery even if the element advertises `AXPress`. Never invoke AX press as a fallback. AX may still be used to resolve an element selector. |

Failure to read capabilities, permission denial, stale identity, or an incomplete search is not evidence that AX press is unsupported and does not authorize switching to mouse input. `auto` never probes support by performing an action. Once any AX invocation or mouse sequence has been attempted, timeout, error, or lack of observed effect never triggers the other method. Results identify the requested method, effective method, and pre-dispatch selection reason so an experiment can reproduce or explicitly override the choice.

AX delivery invokes `AXPress` once. Mouse delivery emits the requested button's down/up sequence at the selected point. Right-click means actual secondary-button events, not control-left-click or `AXShowMenu`; middle-click means the corresponding other-button events. A double-click is one native double-click sequence with correct event count/timing, not two AX invocations or an unrelated pair of single-click commands. Modifier-clicks must preserve modifier semantics in the target without changing the person's hardware modifier state or leaving synthetic modifiers held.

The mouse recipe must prove window routing, local coordinates across display origins/scaling, and no unintended duplicate delivery. Select one evidenced transport before dispatch. Never post through both CG and SkyLight for coverage or try a second transport after an uncertain attempt. Required private interfaces resolve at runtime and fail when absent; their presence alone does not establish support.

#### Scrolling

`ui scroll` requires an explicit location by element or coordinates and accepts signed `--dx` and `--dy` with `--unit pixel|line` (default `pixel`). An omitted axis is zero; at least one must be nonzero. Positive values request viewport progression right/down through the content, negative values left/up, independent of the host's natural-scrolling preference. Translate that direction into the platform event signs rather than pass an ambiguous device convention to callers.

Units describe the scroll input, not a promise that rendered content moves by exactly that many screen pixels or lines. Application scaling, bounds, snapping, and clamping can change the observed displacement. An element selects the event location; do not silently substitute its ancestor or the window's main scroll area. Nested scroll regions need separate target-side evidence.

Send a bounded window-routed wheel sequence without real-pointer motion or focus changes. Do not substitute AX value writes, scrollbar dragging, keyboard arrows, or global wheel input. This is explicit wheel scrolling, not trackpad swipe or pinch. Do not add synthetic inertia after the requested sequence; define any event phase requirements from the tested mechanism and ensure completion/cancellation cannot leave a gesture phase open.

#### Drag and hover

`ui drag` specifies complete `--from-x`/`--from-y` coordinates relative to the source window and `--to-x`/`--to-y` coordinates relative to the independently selected destination window. Each point must satisfy its own window's bounds. Require a positive bounded `--duration-ms`; optional modifiers use the click vocabulary. The operation begins with primary-button input at the source and follows one bounded drag lifecycle to the destination. Record both endpoint geometries and the actual path/timing parameters. Converting between window-local and screen coordinates must account for different display origins and scales without moving the real cursor. A detected geometry change that invalidates the planned endpoint prevents further path/drop delivery rather than silently chasing a new location. Interpolation cadence and duration limits must be fixed before approval.

For a same-window control drag, prove the expected control or selection change. For a drag between windows, preserve the application's drag session, offered items, accepted operation, and destination enter/update/drop lifecycle. This includes transfers between different applications, not merely windows sharing a PID. Mouse-down in the source followed by an unrelated mouse-up in the destination, retargeting every event without preserving drag ownership, or changing a value directly is not a drag-and-drop implementation. A covering or intermediate unselected window must not become the input or drop recipient. If the route cannot honor both bound endpoints without global input capture or a focus-changing fallback, reject before starting; a same-window-only implementation does not complete the feature.

The source application supplies drag data and the destination negotiates and accepts or refuses it through their normal drag-and-drop protocols. Standard drag pasteboard/item-provider exchange and the resulting target-owned copy/move effects are in scope; replacing the drag with clipboard copy/paste, direct file manipulation, or injected calls to destination handlers is not. Do not fabricate a payload or force acceptance of an unsupported type. Destination refusal is not permission to retry or fall back. An accepted file/object drop can have durable effects; a timeout or cleanup failure does not undo them.

`ui hover` sends target-local pointer-motion input at the selected location without button or key input and without moving the real pointer. Prove the intended target's hover/tracking effect, not just receipt of a mouse-move event. A target that requires the real global pointer at that location is unsupported; cursor warping and restoration are not alternatives. Subsequent application timers and tooltips are not proof of completed caller-level intent, nor are they automatically dismissed.

Right-click may initiate context-menu tracking, and drag may initiate a tracking session. A menu, tooltip, or dragging effect is not permission to raise or make a window key, capture host input, or suppress the guard. Test those ongoing effects as well as the event sequence itself. Known focus/input-capturing routes fail before dispatch; unexpected interference fails without automatic menu dismissal or desktop restoration. Unsupported-route rejection remains necessary safety behavior, not a substitute for positive action evidence.

### 3. Selected-window key chords

`ui key` takes one key chord and an explicit window. The initial requirement includes a background application's secondary/non-key window, not just its current key window. The intended responder is that selected window's existing responder; Recon does not focus a different element to make a chord work. The same-process nonselected window and a separate foreground input receiver must not receive the chord or its effects.

A chord consists of zero or more distinct supported modifiers and exactly one supported base key. Reject unknown tokens, duplicate modifiers, modifier-only input, or a sequence of chords before any posting. Key names, aliases, keyboard-layout translation, and the supported base-key set must be settled in this draft before approval; text insertion and arbitrary Unicode payloads are not part of key chords.

Sending a key-down/up pair to the selected PID, stamping an unvalidated window number, successfully returning from a post function, or temporarily making the requested window key does not satisfy this section. Application-wide menu shortcuts also require target-side evidence that the intended window's action occurred; receiving a key event with the right numeric field is insufficient.

Use one researched delivery mechanism per supported capability. If selected-window delivery cannot be established under the no-focus contract, reject the request before dispatch. This rejection is required safety behavior but does not complete the feature: selected-window key actuation on both AppKit and Catalyst remains mandatory. Missing feasibility requires a human revision, not a silent process-only cutover.

### 4. Permissions, interference, and execution outcomes

Continue using Recon's executable-owned privacy identity. AX lookup/press requires its Accessibility permission; event synthesis requires the actual event-posting permission checks for the selected mechanism. A coordinate-only mouse operation does not acquire an AX lookup requirement merely because element clicks share its command. A method override does not bypass the permissions of its chosen route, and missing grants do not select a different method. Additional observation/listening permissions needed to establish the guard are explicit prerequisites, not assumed from another grant. Normal calls may request a missing permission under the existing setup exception, then fail with `PERMISSION_DENIED` without sending input. `context` and existing preflight commands remain non-prompting. Do not edit TCC, weaken host protections, or run injection merely to acquire permission.

The no-focus boundary is unchanged: no target activation or raise, key-window transfer, system keyboard-focus transfer, Space switch, real-pointer movement, or diversion of host input. All actions enter the shared guarded path. For drag, the guard covers both source and destination and distinguishes their roles in attribution; it cannot watch only the source PID or treat the destination's events as an unexamined exemption. The intended change in either application's own state is not a broad exemption for window ordering, event counters, or target-side activation.

Before action work can be approved, specify and validate an action-aware observation design that distinguishes the requested target-directed events from leaked session input. Do not merely disable the synthetic-input check or subtract the number of posted events. Evidence must observe actual key-window/focus transitions, not infer them from z-order, and retain target-caused transitions even when unrelated hardware input occurs in the same interval. If required observation is unavailable, fail before dispatch rather than advertise an unverified clean run. The existing poller remains useful evidence, not sufficient proof by itself.

On detected interference, return `UI_INTERFERENCE` with stage, evidence source, affected process/window where known, and whether target execution may continue. Do not restore focus, pointer, or Spaces. Stop unsent interaction steps. For a partially sent click, chord, drag, or phased scroll, only the matching release/termination events needed to avoid leaving synthetic input held or a phase open may still be sent, through the established route to the bound live participants actually holding that state. Do not post releases blindly to both drag endpoints. If either endpoint disappears, do not continue the path or substitute another window; perform only evidenced safe cleanup on surviving participants and report any uncertainty. Never complete a drag path merely as cleanup, release through the global queue, or post to a replacement process. Releasing a drag can itself cause a drop; it is not rollback. No arbitrary callback or already-open menu can be cancelled safely by assuming that sending Escape restores the prior state.

A successful response describes what Recon established: selected bundle/process incarnation/window, requested operation and parameters, chosen mechanism, and whether an AX action returned or a synthetic sequence was posted. Drag reports both source and destination identities, their local coordinates, actual path/timing, and any observed handoff, drop acceptance/refusal, operation, or completion evidence; unobserved acceptance and data transfer remain explicitly unverified. Mouse-up delivery alone is not a confirmed drop. Click results additionally report the requested/effective method and selection reason. Scroll units/deltas and release/phase-completion uncertainty remain explicit. A response does not claim that a save, bounce, file transfer, or other caller-level goal completed without separate evidence. The caller can use existing read commands afterward; no new automatic assertion language is introduced.

Reuse selection, permission, interface, and interference failures where their meanings apply. Add schema-published action classifications for unavailable action/scope, ambiguous element, changed target identity, failed action, and uncertain action timeout; proposed spellings are `ACTION_UNAVAILABLE`, `AMBIGUOUS_ELEMENT`, `STALE_TARGET`, `ACTION_FAILED`, and `ACTION_TIMEOUT`. Action failure details distinguish no dispatch from partial or unknown execution and identify the mechanism and selected window; drag preserves both endpoint identities and names the source, destination, or handoff/cleanup stage that failed. Post-dispatch timeout must not reuse the currently retryable observation `TIMEOUT` classification. No mutating attempt, ambiguous completion, output-delivery failure, or interference result is automatically retried. No exactly-once guarantee is made across separate invocations.

The final registered schemas must specify these details, the stage used for explicit actions, and bounded dispatch/release deadlines before implementation approval. Existing observation envelopes and their retry semantics are not changed incidentally.

### 5. Required support and evidence

The proposed initial host boundary is native Apple silicon on macOS 27, with an active unlocked login session. Record the exact tested host builds, target versions, architecture, signing identity, grants, and SIP/AMFI configuration. Evidence obtained only with relaxed security cannot establish stock-host support. Support for a macOS version is based on behavioral evidence and its explicit compatibility rules, not successful symbol lookup or extrapolation from macOS 15.7.

| Operation | AppKit required outcome | Mac Catalyst required outcome |
| --- | --- | --- |
| Unified click | Plain element click works by AX and forced mouse; coordinate and right/middle/double/modifier variants produce their intended behavior without duplicate dispatch | The same independently instrumented control and mouse-gesture outcomes occur in the selected Catalyst window/scene |
| Scroll | Horizontal/vertical, both directions and both units affect the region under the selected location, including a nested region | The equivalent selected Catalyst scroll region responds without a different scene scrolling |
| Drag | Same-window control/selection drag and actual accepted data transfer between selected windows, including separate applications, without host-input capture | Equivalent same-window and cross-window/application behavior in real Catalyst scenes, including mixed AppKit/Catalyst transfers in both directions |
| Hover | The selected control's hover/tracking state changes without a click or real-pointer motion | The equivalent Catalyst pointer-interaction state changes in the selected scene |
| Key chord | The selected window's existing responder or window-scoped command acts, including a secondary/non-key window | The selected Catalyst window/scene's responder or command acts under the same scope requirement |

For each row, cover an already-frontmost target, a background target, a secondary/non-key window in that process, and an occluded target on the current Space. Each target has at least two windows with independently recorded effects so incorrect-window delivery cannot pass. The required outcome includes no new activation, raise, key-window change, Space switch, real-pointer motion, or host-input diversion. Minimized/off-Space windows, locked sessions, secure-input cases, and unvalidated target/host combinations are initially unavailable; they do not trigger a focus-changing fallback. Additional states need their own evidence and explicit design revision.

Drag evidence must cover same-window operation, different windows of one instance, separate instances of the same bundle, and different applications. Cross-application pairs include AppKit to AppKit, Catalyst to Catalyst, AppKit to Catalyst, and Catalyst to AppKit. Exercise both endpoints in the background, secondary/non-key endpoints, overlapping/occluded windows, and different display origins/scales while a third unrelated foreground receiver retains host input. Verify an identifiable offered payload reaches exactly the selected destination, the destination's accepted operation matches the resulting source/destination state, and an incompatible destination refuses without unintended transfer or source data loss. Include a fixture-owned file payload as well as an identifiable text/object payload; a synthetic button counter or endpoint event log alone is insufficient evidence of a transfer.

Build a real Mac Catalyst fixture, not a Simulator app or an AppKit fixture with a new label. Fixture-local instrumentation may report control counters, received event/window identities, responder effects, and scene/window transitions. That instrumentation is an independent test oracle, not a helper dependency of the released external action. Distinguish toolkit identity from coincidental use of an AppKit window wrapper.

Use a separate foreground receiver and continuous/event-trace evidence, with an acknowledged observation start before dispatch and a defined post-dispatch observation boundary. Verify that concurrent host input remains with that receiver, requested synthetic events do not leak to it, and unselected windows do not act. Check the action-specific semantic outcome: exactly one AX/single-click activation, a correctly recognized double/modifier/right/middle gesture, the intended scroll region/direction, the expected same-window drag state or cross-window destination acceptance/data transfer, and hover without a press. Post-return logs are not the oracle. Include source and destination drag lifecycle, context-menu, tooltip, and tracking behavior in the observation interval; define how persistent target effects are judged without silently dismissing them. Capture raw traces with exact reproduction commands and environmental facts in the readiness evidence.

Negative controls must demonstrate that the observation setup detects a brief activate-and-restore, key-window change without raise, pointer move-and-restore, Space change, and diverted input; it must not misclassify the fixture's deliberate hardware input as target action. Test simultaneous target-caused interference and host input rather than treating hardware activity as a blanket waiver. Record the sampling/notification coverage and loss behavior; no finite trace establishes that target code can never act later.

Permanent regressions cover wrong process/window, duplicate AX identifiers/titles, disabled/unsupported elements, partial AX searches, stale/replaced elements, `exec`/exit/PID reuse, missing symbols/grants, invalid coordinates/chords, wrong-window delivery, and uncertain execution. Specifically prove pre-dispatch automatic method selection, forced-method refusal, contradictory AX options, no mouse retry after an AX side effect/timeout, correct button/click count/modifiers, both scroll axes/signs/units and nested-region targeting, balanced drag/scroll cleanup, hover without clicking, duplicate-transport prevention, and context-menu/tracking interference during concurrent host input. Drag adds invalid-destination-before-down, endpoint/session mixups, changed endpoint geometry, rejected drops, loss of either endpoint before and after handoff, cancellation before acceptance, and lost acknowledgment after real data transfer. Tests assert effects and rejection boundaries, not parser wiring or mock forwarding.

### 6. Integration and delivery

Keep AX action work in the accessibility capability and synthetic input in a focused macOS capability boundary, depending inward on portable core contracts. Share process incarnation representation/querying where necessary without importing native injection into external interaction. The CLI owns selection, registry entries, orchestration, JSON rendering, and help; the native inspection helper remains unchanged unless a separately approved design requires otherwise. Do not add a generic scripting endpoint, plugin system, or alternative interaction executable.

Every command is registered through the existing registry with truthful mutation, target-code execution, permission-request, and no-focus effects. Observation commands stay read-only. Publish the action error schemas and update `context` with the actual non-prompting input prerequisites and required interfaces; do not describe grants or symbols as actuation support.

After the human resolves this draft and sets `awaiting-implementation`, derive behavioral tests first, then write an interaction guide from this design, then implement. Add a named native interaction task, proposed as `mise run test:interaction`, for the AppKit/Catalyst matrix and traces; keep live-Mac checks out of Linux CI rather than faking success there. Require `mise run check`, existing affected native regressions including `mise run test:focus`, the new live action task, a real CLI smoke, and independent implementation review before readiness. A passing general gate or rejected unsupported key command cannot substitute for the positive matrix.

## Compatibility

This is additive command behavior. `ui ax tree`, `ui view`, `ui windows`, `ui screenshot`, session lifecycle, and correlation retain their observation and identity contracts. Interaction uses `ui click`, `ui scroll`, `ui drag`, `ui hover`, and `ui key`; no separate `ui ax press` command or alias is introduced. There is no top-level `ax`, revived `ui tree`, or automatic promotion of an observation node into an action handle. Existing callers do not begin sending input. Earlier draft spellings and issue examples were never shipped and need no compatibility aliases.

No protocol change, new helper attachment, or stored action session is required merely to use external actions. The proposal does not grant authority to change host security settings. New action error codes and schema metadata are additive; existing observation errors retain their meaning.

## Implications on adoption

Users must have the necessary grants for the actual Recon executable and an unlocked supported session before the uninterrupted-action guarantee applies. Homebrew's ad-hoc signature and re-granting behavior remain as decision.0005 describes; optional local Developer ID signing does not change the action contract. A command can cause target-side effects even when a timeout or later interference makes its result a failure. Callers must observe state before deciding whether to repeat it.

Private input routing increases macOS compatibility maintenance. The exact supported builds, target families, normal/research-host prerequisites, and refusal cases must be documented from the evidence. Older releases and Intel/Rosetta are not inferred supported. Removing the preview or returning to a released Recon disables the new commands; it does not undo actions already performed in an application.

## Scope

- Unified window-scoped click with automatic/forced AX or mouse delivery, left/right/middle buttons, single/double count, and modifiers; horizontal/vertical wheel scrolling; primary drag within a window and between independently selected windows/instances/applications; hover; and single key chords, with shared identity and explicit mechanism outcomes.
- AppKit and actual Mac Catalyst actuation, including selected-window key delivery to background secondary/non-key windows.
- Permission checks, bounded failures and release behavior, action-aware no-interference evidence, registry/schema integration, guide, and private CLI preview required for these operations.

### Out of scope

- Trackpad pinch/zoom, rotation, and swipe, preserved by explicit owner request in [#92](https://github.com/markmals/recon/issues/92). Wheel scrolling is in scope; it does not stand in for those gestures.
- Text insertion, triple-clicks, right/middle-button drags, held keys, arbitrary input sequences/macros, direct general-clipboard manipulation, and automatic retry. Normal drag data exchange and target-owned copy/move effects are in scope, including between applications.
- Arbitrary AX actions or attribute setters, native method invocation, injected interaction, runtime introspection, and general code evaluation.
- Activating/reordering/focusing targets, including supposedly temporary or focus-without-raise modes.
- Simulator, connected devices, Windows, Intel/Rosetta support, and general Catalyst native-view inspection. Catalyst external interaction remains in scope and does not wait on #55.
- Claims that arbitrary application callbacks cannot activate UI, or that an event-post receipt proves the user's application-level goal completed.

## Preview

- Artifact: a privately staged, installable macOS Recon executable from this branch, alongside its build identity and the AppKit and Mac Catalyst fixture applications. Preserve the normal matched-artifact layout if exercising existing native inspection regressions; interaction itself must work without a resident helper.
- Access: use a separate local preview prefix rather than replace the user's Homebrew installation. The eventual readiness report names the exact executable path, signing/grant setup, fixture paths, reproduction commands, and recorded traces. No public release or deployment is authorized by this draft.
- Exercise: run each action and click-method override with distinguishable target windows while a separate foreground receiver retains input. Include right-click/context-menu behavior, nested scrolling, hover, and both same-window and cross-window/application drag. For drag, observe source state, destination acceptance, actual text/object/file transfer, and cleanup or refusal; include mixed AppKit/Catalyst directions. Then exercise an unsupported route and a controlled interference failure.
- Cleanup: stop only preview-owned fixture processes and observers and remove only preview-owned files when review ends. Do not close unrelated applications, alter TCC directly, or restore the user's desktop through synthetic input.
- Reason: an executable and independently instrumented real target apps are the cheapest preview that can distinguish actual actuation from a plausible command/schema design. A documentation preview alone cannot approve the implementation.

## Policies and decisions checked

- `VISION.md` — controlled experiments, explicit effects, evidence provenance, and unsupported capabilities stated honestly; issue briefs are not implementation approval.
- [policy.0001](../policies/0001-cli-command-and-error-contract.md) — registry-owned commands and schema-published typed failures, including new action-specific failures rather than retryable observation timeouts.
- [policy.0004](../policies/0004-ui-no-focus.md) — unchanged no-focus boundary, guarded application-directed operations, no restoration, and explicit setup exceptions. A stronger action evidence mechanism enforces this policy rather than silently weakening it. The old policy.0003 is superseded, not a competing rule.
- [decision.0005](../decisions/0005-homebrew-ad-hoc-privacy-identity.md) — current executable-owned identity, ad-hoc Homebrew grants, and non-prompting context. It retains the permission-request behavior of superseded [decision.0002](../decisions/0002-macos-context-privacy-identity.md); the setup exception remains disclosed.
- [decision.0003](../decisions/0003-recon-owned-native-loader.md) — no broader helper execution authority or automatic protection changes. Reusing incarnation identity does not make the loader an external-action prerequisite.
- [decision.0004](../decisions/0004-homebrew-bottle-releases.md) — no alternate public distribution channel or release from this proposal; the preview is private and preserves the matched-artifact contract where applicable.
- [proposal.0005](0005-native-macos-inspection.md#5-shared-ui-integration-no-focus-behavior-and-ax-correlation) — source-specific effects, fresh target/window validation, optional session assertions, no mandatory injection for external actions, and successful post distinguished from actuation. The human's selected-window keyboard requirement here is stronger than accepting a process-only operation.

No existing policy is superseded by this draft. A feasibility result that requires focus transfer or mandatory injection conflicts with the selected design and must return to the human, not be encoded as a fallback.

## Future directions

Trackpad pinch/zoom, rotation, and swipe are tracked separately in [#92](https://github.com/markmals/recon/issues/92), with their own phase, routing, and no-interference research. They do not block this proposal, and the pointer interactions required here are not deferred to that issue. Other supported window states could be proposed after independent evidence. A process-directed key command could be a distinct future capability, but cannot satisfy this proposal's selected-window requirement.

## Alternatives considered

- **Separate AX press and mouse click commands.** Exposes a backend distinction as two interfaces for the same ordinary control-click intent. Rejected in favor of one `ui click` command, a pre-dispatch automatic rule, explicit overrides, and mechanism reporting. Shared naming does not erase differences for mouse-specific gestures.
- **Process-directed keyboard first.** Matches the known winctl key mechanism and could honestly reject `--window`. The human rejected it as sufficient: selected-window key delivery is required now, and uncertainty remains a blocker rather than a scope reduction.
- **Mouse-only or mandatory explicit selection for every click.** Makes physical gesture semantics uniform, but loses the straightforward semantic-action path or burdens every ordinary element click with a backend decision. The selected automatic default remains reproducible through its reported method and explicit override.
- **Include every input gesture now.** Trackpad gestures, text, and holds have additional interpretation, routing, and cleanup contracts. The selected scope includes usual pointer interactions and cross-window/application drag-and-drop; trackpad gestures have their own requested follow-up rather than being silently equated with scrolling.
- **Restrict drag to one window or one application.** Simpler addressing and event ownership, but explicitly rejected: source and destination must be independently selectable across windows and applications. Missing cross-process drag feasibility remains a research blocker, not a reason to declare same-window dragging complete.
- **Copy winctl wholesale.** Its private mouse routing is useful prior art, but its target selection, PID-only keyboard path, dual-post mouse path, research-host assumptions, and endpoint-only pointer comparison do not establish Recon's contract.
- **Activate without raise, then restore, or use global input.** Changes the user's focus/input even if the desktop looks identical later. Violates policy.0004 rather than providing a fallback.
- **Require an injected helper for every action.** Excludes supported external use, adds unrelated access prerequisites, and conflicts with #24's separation from native inspection. Not chosen.

## Open questions

- [NEEDS CLARIFICATION: Which external mechanisms meet the full required action matrix on AppKit and Mac Catalyst, especially selected-window keyboard delivery to background secondary/non-key windows, cross-window/application drag with actual accepted data transfer in both mixed-framework directions, and the other pointer variants? Record reproducible probes, semantic effects, independent interference traces, exact host/security/permission conditions, target-family detection, incarnation-binding/race limits, and the supported-build matrix. Same-window drag, unrelated down/up delivery, PID-only posting, and rejection-only implementations do not establish the required outcomes.]
- [NEEDS CLARIFICATION: What concrete action-aware guard and trace design distinguishes intended target-directed events from diverted host input, observes transient key/focus/Space changes, and preserves attribution during concurrent hardware input? Demonstrate positive/negative controls for both drag endpoints and handoff, context-menu/drag tracking, tooltip/hover and persistent target effects, startup/observation boundaries, loss handling, permissions, and limitations before treating no-interference as established.]
- [NEEDS CLARIFICATION: Settle the key-name/layout contract and complete action response/error schemas, numeric lookup/dispatch/release deadlines, and partial-release semantics from the selected mechanisms. Also fix scroll delta bounds and phase encoding, double-click timing, drag duration/interpolation, source/destination handoff and accepted-operation reporting, and safe cleanup when either endpoint disappears or drop completion is uncertain. Cross-window and cross-application dragging are required scope, not an open scope choice; remaining contract choices must be precise before tests and implementation can derive from them.]

## Acknowledgments

winctl's published hit-testing experiments distinguish delivery from actuation and provide the private mouse-routing starting point. Apple's SDK contracts establish the public API and timeout boundaries. Mark selected the scope and required window-directed keyboard delivery; the unresolved research must meet that requirement rather than redefine it.
