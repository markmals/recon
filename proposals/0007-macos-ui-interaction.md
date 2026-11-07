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

Add explicit external accessibility presses, synthetic clicks, and key chords to Recon's shared `ui` surface. Actions must reach the selected window without disturbing the user's focus, pointer, Spaces, or input. AppKit and Mac Catalyst require independent actuation evidence. Selected-window keyboard delivery is required, not replaceable by process-only posting.

## Motivation

Recon can inspect an application's interface but cannot perform the next step in an investigation: press an observed control or invoke a shortcut and observe what changes. An agent should be able to investigate a background application while the person using the Mac continues working elsewhere. Bringing the target forward, operating it, and restoring the desktop does not satisfy that goal: the interruption itself is the failure.

Issue [#24](https://github.com/markmals/recon/issues/24) identifies winctl as prior art and separates explicit actions from the completed observation work in #15. Its example commands express the desired workflow, not an approved grammar. Native in-process inspection is not a prerequisite for an external action. Mac Catalyst is a required target family, not an inference from an AppKit demonstration.

The human selected external AX press, synthetic clicks, and key chords for this proposal, excluding text entry, scrolling, dragging, and held keys. They also required selected-window keyboard delivery, including secondary/non-key windows, rather than accepting a process-directed first version. Those decisions bound the design below. This draft does not claim that a compliant keyboard mechanism has already been found.

## Domain grounding

### Established context

- Apple's [AXUIElementPerformAction](https://developer.apple.com/documentation/applicationservices/1462091-axuielementperformaction) requests an action on an accessibility object. The installed Xcode 27.1 `HIServices/AXUIElement.h` contract, lines 314–331, explicitly says `kAXErrorCannotComplete` can occur when an action callback performs modal processing and does not return before the messaging timeout; it does not necessarily mean the action failed. Retrying a mutating operation can therefore duplicate its effects.
- Apple's [CGEvent.postToPid](<https://developer.apple.com/documentation/coregraphics/cgevent/posttopid(_:)>) takes a PID, not a window identity, and returns no delivery acknowledgment. The installed `CoreGraphics/CGEvent.h`, lines 356–374, describes application event-stream routing. This establishes neither selected-window delivery nor application-level actuation.
- The same header separates non-prompting `CGPreflightPostEventAccess` from potentially prompting `CGRequestPostEventAccess`, and likewise distinguishes event-listening preflight from its request. An Accessibility grant or a successful post on a weakened research host is not sufficient evidence for the normal event-posting permission contract.
- [winctl's pinned hit-testing experiment](https://github.com/NSExceptional/winctl/blob/295a02751b2cb0c958b476e6ce6864963ee4042f/research/hittest/README.md) records a background AppKit button actually firing on macOS 15.7, Apple silicon, SIP disabled and AMFI relaxed. A bare mouse post reached the process with window number zero but did not actuate the button. Setting private routing field 51 plus mouse fields made it actuate. The experiment does not establish Catalyst behavior or behavior on macOS 27.
- That experiment distinguishes an unlocked session from a password-locked screen. Its synthetic input stopped arriving after lock. It also distinguishes focus-without-raise from genuinely background delivery. Neither a screensaver demonstration nor an unmoved cursor proves preservation of keyboard focus.
- [winctl's implementation at the same revision](https://github.com/NSExceptional/winctl/blob/295a02751b2cb0c958b476e6ce6864963ee4042f/winctl.m) uses PID-only posting for `key` even after selecting a window. Its mouse path posts each event through both `SLEventPostToPid`, when available, and `CGEventPostToPid`. The former is not evidence of selected-window keyboard delivery; the latter must not be copied as a mechanism that might double-actuate.
- The [upstream cua keyboard implementation](https://github.com/trycua/cua/blob/1759af24ec84685269529425ac42e5de964adf85/libs/cua-driver/rust/crates/platform-macos/src/input/keyboard.rs), inspected during this draft with blob identity `1553fda86e6c1bae13dbf880b5786a7ad37bf7ca`, exposes PID-directed key operations. It separately describes global-HID helpers as requiring foreground activation. Those helpers are not a compliant fallback for Recon. These are source observations, not independent execution evidence.

### Relevant constraints and principles

An application bundle, a process incarnation, a WindowServer window, an AX element, and a native view are different things. A bundle can have several running instances; a process can own several windows; an AX label or identifier is not necessarily unique. A historical tree path is an observation location, not a durable action handle. The same numeric PID can name a different incarnation after exit/reuse, and this project's recorded research also observes an incarnation change across `exec` without a PID change.

AX press is a semantic action supplied by an application. Synthetic click is a mouse gesture at a point. A key chord is keyboard input whose interpretation depends on the receiving window, responder, menu handling, and keyboard mapping. They must not become interchangeable fallbacks under a generic success flag. Selecting a window, delivering an event, invoking a callback, and achieving the caller's application-level goal are separate facts.

Target-internal responder changes can be intended consequences of an explicit action. Changing which application or window receives the person's real input is not. Application callbacks can themselves activate UI or keep executing after the requester times out; an external client cannot make arbitrary target code pure or roll it back.

### Quality bar

For every supported action, evidence must show both the intended effect in the selected window and preservation of the host user's input and focus throughout the operation. Two unchanged endpoint snapshots, a successful API return, a delivered event without an effect, or an AppKit run labeled Catalyst are insufficient. Unsupported scope must be rejected before input, not ignored. Unknown execution after a timeout must remain unknown, without automatic retry.

### Remaining uncertainty

There is no new action-execution evidence from this proposal yet. On the research host, `sw_vers` reported macOS 27.0.1, build 26A434, and `xcodebuild -version` reported Xcode 27.1, build 27A9269. `mise run dev -- context` succeeded without prompting and reported that this worktree's executable was responsible for itself but lacked both Accessibility and Screen Recording grants. No permission request, security modification, or UI action was performed as part of that context check.

The separately installed Homebrew Recon 0.1.5 also returned a non-prompting context report, with Accessibility and Screen Recording granted to its installed executable. Those grants do not transfer to this worktree or to a new research controller, and that released executable does not implement these actions. The installed binary was not replaced or re-signed.

The required selected-window keyboard mechanism, its stock-host permission requirements, its incarnation-binding limits, and the event-trace design remain research questions. Symbol availability alone cannot resolve them. The support matrix below is a required outcome, not a report of current support. The draft stays unapprovable until the Open questions are resolved with evidence and concrete design.

## Existing baseline

Recon already supplies strict bundle/path/name selection, instance disambiguation, window ownership checks, optional session identity assertions, executable-owned privacy identity, typed errors, and registry-derived help and schemas. Preserve these contracts rather than adopting winctl's substring/largest-window selection. Relevant seams are `recon-applications-macos/src/selection.rs`, `recon-window-server-macos/src/window.rs`, and `recon/src/observe/target.rs` under `crates/`.

The external AX crate already reads role, title, identifier, enabled state, frame, and advertised actions. `ui ax tree` does not provide durable AX action handles, and the correlation path is not an executable identity. The native session's incarnation checks are useful precedent, but external selection currently returns a PID without binding an incarnation. Reusing an identity concept must not introduce a session or injection requirement.

`crates/recon/src/observe.rs` wraps application-directed UI work in the shared guard. `crates/recon-ui-guard-macos/src/focus.rs` samples roughly every ten milliseconds. Its window-order measurement is not an observation of actual key-window state; foreground, window-order, and Space snapshots can miss changes entirely between samples. Its hardware attribution treats an interval containing hardware input as concurrent user activity. That cannot, by itself, prove that an action caused no simultaneous disturbance. Whether the proposed process-routed events increment its synthetic-input counters has not been measured. These are action-verification constraints, not reasons to disable the guard.

The existing `ReconFixture` is AppKit. There is no Catalyst fixture or live action task in this baseline. The new work must provide those missing proofs rather than relabel the observation tests. Existing native inspection's exclusion of Catalyst does not exclude Catalyst from these external actions or make #55 a whole-feature prerequisite.

## Proposed solution

Expose mechanism-specific operations under the existing shared UI namespace:

```sh
recon ui ax press "Logic Pro" --window 123 --role AXButton --title Bounce
recon ui click "Logic Pro" --window 123 --role AXButton --title Bounce
recon ui click "Logic Pro" --window 123 --x 160 --y 84
recon ui key "Logic Pro" "cmd+b" --window 123
```

These are proposed spellings, not shipped commands or evidence that Logic Pro supports any particular action. AX press invokes the advertised semantic press; click sends one synthetic left click; key sends one chord to the selected window. They share selection and failure conventions, not mechanisms or implicit fallbacks.

Prefer external APIs and runtime-resolved private interfaces where independently proven. A missing capability is an explicit failure. An injected helper, an AX focus write, global HID posting, focus-without-raise, activation followed by restoration, and cursor restoration are not alternatives that complete this proposal. A process-only keyboard command is not a substitute for its selected-window requirement.

## Detailed design

### 1. Command surface and target identity

All three operations select an already-running application through the existing target grammar. `--pid` disambiguates a bundle's instances. Each requires an explicit `--window` belonging to that instance; none infers the largest, frontmost, main, or key window. No command launches the target. Existing ambiguity, missing-target, not-running, and wrong-owner failures remain failures with no input.

An optional `--session` is only an additional identity assertion, following the existing shared UI contract. Without it, every supported action must work externally. A stale or mismatched supplied session fails; it is not ignored, reopened, or attached as a fallback. Reading an assertion must not dispatch an injected action.

Bind the selected process incarnation and window before resolving the action, and revalidate immediately before dispatch and before any later release event. An observed exit, `exec`, owner mismatch, replaced selection, or disappeared window prevents further action on that identity. Do not resolve a new process or window and continue. A historical AX path, native node handle, or correlation candidate is not accepted as an action target.

A PID check immediately before a PID-only post is not an atomic incarnation-bound dispatch. The approved design must state the actual binding mechanism and residual race boundary instead of claiming that repeated checks eliminate it. This is part of the unresolved mechanism research, not permission to omit identity checks.

### 2. AX press and synthetic click

`ui ax press` takes one or more of `--identifier`, `--role`, and `--title`. Supplied filters combine with AND and match the corresponding published AX fields exactly and case-sensitively. There is no fuzzy title search, title/description fallback, or first-match behavior. Role values are the AX role strings, such as `AXButton`; this does not introduce a second role vocabulary.

Resolve a fresh AX subtree rooted at the selected window. The entire bounded candidate search must establish one matching element: zero matches is not found, two or more is ambiguous, and an incomplete or timed-out traversal cannot establish uniqueness even if one match was seen. Reuse the existing bounded-read discipline, but never treat omitted/error subtrees as proof that no second match exists. Keep the resolved live element for this request only. Immediately before actuation, verify it still belongs to the selected window, still matches, is enabled, and advertises `AXPress`. Unknown enabled state or unverifiable membership is unavailable, not permission to guess. Invoke `AXPress` once. Unsupported press does not fall back to a synthetic click or an attribute write.

`ui click` accepts either the same AX filters or a complete `--x`/`--y` pair, never both. It performs exactly one left-button down/up gesture; double/right clicks, dragging, and hover are outside this proposal. Coordinate input is finite logical points relative to the top-left of the WindowServer window bounds, including the frame, not screenshot pixels or AppKit's bottom-left content coordinates. Require `0 <= x < width` and `0 <= y < height`; do not clamp or move the target. Resolve bounds freshly and preserve the selected window while converting coordinates.

For element-form click, require one live, enabled match within the selected window and a finite, nonempty frame whose center falls inside that window. Click that center after revalidation; geometry is not a substitute for uniqueness. Missing or stale geometry fails rather than switching to AX press. Occlusion by another application's windows must not redirect the click to the covering application.

The mouse recipe must prove window routing, correct local coordinates across display origins and scaling, and exactly one control actuation. Select one evidenced transport before dispatch. Never post through both CG and SkyLight for coverage, and never try a second transport after an uncertain first attempt. Required private interfaces are resolved at runtime and reported unavailable if absent; their presence alone does not establish support.

### 3. Selected-window key chords

`ui key` takes one key chord and an explicit window. The initial requirement includes a background application's secondary/non-key window, not just its current key window. The intended responder is that selected window's existing responder; Recon does not focus a different element to make a chord work. The same-process nonselected window and a separate foreground input receiver must not receive the chord or its effects.

A chord consists of zero or more distinct supported modifiers and exactly one supported base key. Reject unknown tokens, duplicate modifiers, modifier-only input, or a sequence of chords before any posting. Key names, aliases, keyboard-layout translation, and the supported base-key set must be settled in this draft before approval; text insertion and arbitrary Unicode payloads are not part of key chords.

Sending a key-down/up pair to the selected PID, stamping an unvalidated window number, successfully returning from a post function, or temporarily making the requested window key does not satisfy this section. Application-wide menu shortcuts also require target-side evidence that the intended window's action occurred; receiving a key event with the right numeric field is insufficient.

Use one researched delivery mechanism per supported capability. If selected-window delivery cannot be established under the no-focus contract, reject the request before dispatch. This rejection is required safety behavior but does not complete the feature: selected-window key actuation on both AppKit and Catalyst remains mandatory. Missing feasibility requires a human revision, not a silent process-only cutover.

### 4. Permissions, interference, and execution outcomes

Continue using Recon's executable-owned privacy identity. AX lookup/press requires its Accessibility permission; event synthesis requires the actual event-posting permission checks for the selected mechanism. Additional observation/listening permissions needed to establish the guard are explicit prerequisites, not assumed from another grant. Normal calls may request a missing permission under the existing setup exception, then fail with `PERMISSION_DENIED` without sending input. `context` and existing preflight commands remain non-prompting. Do not edit TCC, weaken host protections, or run injection merely to acquire permission.

The no-focus boundary is unchanged: no target activation or raise, key-window transfer, system keyboard-focus transfer, Space switch, real-pointer movement, or diversion of host input. All actions enter the shared guarded path. The intended change in a target's own responder state is not a broad exemption for window ordering, event counters, or target-side activation.

Before action work can be approved, specify and validate an action-aware observation design that distinguishes the requested target-directed events from leaked session input. Do not merely disable the synthetic-input check or subtract the number of posted events. Evidence must observe actual key-window/focus transitions, not infer them from z-order, and retain target-caused transitions even when unrelated hardware input occurs in the same interval. If required observation is unavailable, fail before dispatch rather than advertise an unverified clean run. The existing poller remains useful evidence, not sufficient proof by itself.

On detected interference, return `UI_INTERFERENCE` with stage, evidence source, affected process/window where known, and whether target execution may continue. Do not restore focus, pointer, or Spaces. Stop unsent work. For a partially sent down/up gesture, only the matching release events needed to avoid leaving synthetic input held may still be sent, through the same verified route to the same live identity; never release through the global input queue or to a replacement process. Report when release could not be established. No arbitrary callback can be cancelled or rolled back by this cleanup.

A successful response describes what Recon established: selected bundle/process incarnation/window, requested operation, chosen mechanism, and whether an AX action returned or a synthetic sequence was posted. It does not claim that a save, bounce, dialog, or other caller-level goal completed. Report application-level outcome as unverified unless separate observation actually establishes it; the caller can use existing read commands afterward. No new automatic assertion language is introduced.

Reuse selection, permission, interface, and interference failures where their meanings apply. Add schema-published action classifications for unavailable action/scope, ambiguous element, changed target identity, failed action, and uncertain action timeout; proposed spellings are `ACTION_UNAVAILABLE`, `AMBIGUOUS_ELEMENT`, `STALE_TARGET`, `ACTION_FAILED`, and `ACTION_TIMEOUT`. Action failure details distinguish no dispatch from partial or unknown execution and identify mechanism and selected window. Post-dispatch timeout must not reuse the currently retryable observation `TIMEOUT` classification. No mutating attempt, ambiguous completion, output-delivery failure, or interference result is automatically retried. No exactly-once guarantee is made across separate invocations.

The final registered schemas must specify these details, the stage used for explicit actions, and bounded dispatch/release deadlines before implementation approval. Existing observation envelopes and their retry semantics are not changed incidentally.

### 5. Required support and evidence

The proposed initial host boundary is native Apple silicon on macOS 27, with an active unlocked login session. Record the exact tested host builds, target versions, architecture, signing identity, grants, and SIP/AMFI configuration. Evidence obtained only with relaxed security cannot establish stock-host support. Support for a macOS version is based on behavioral evidence and its explicit compatibility rules, not successful symbol lookup or extrapolation from macOS 15.7.

| Operation | AppKit required outcome | Mac Catalyst required outcome |
| --- | --- | --- |
| AX press | One selected control's action occurs in the selected window | The equivalent independently instrumented UIKit control action occurs in the selected Catalyst window/scene |
| Synthetic click | One selected control actuates at the intended window-local point | One selected Catalyst control actuates through the evidenced external route |
| Key chord | The selected window's existing responder or window-scoped command acts, including a secondary/non-key window | The selected Catalyst window/scene's responder or command acts under the same scope requirement |

For each row, cover an already-frontmost target, a background target, a secondary/non-key window in that process, and an occluded target on the current Space. Each target has at least two windows with independently recorded effects so incorrect-window delivery cannot pass. The required outcome includes no new activation, raise, key-window change, Space switch, real-pointer motion, or host-input diversion. Minimized/off-Space windows, locked sessions, secure-input cases, and unvalidated target/host combinations are initially unavailable; they do not trigger a focus-changing fallback. Additional states need their own evidence and explicit design revision.

Build a real Mac Catalyst fixture, not a Simulator app or an AppKit fixture with a new label. Fixture-local instrumentation may report control counters, received event/window identities, responder effects, and scene/window transitions. That instrumentation is an independent test oracle, not a helper dependency of the released external action. Distinguish toolkit identity from coincidental use of an AppKit window wrapper.

Use a separate foreground receiver and continuous/event-trace evidence, with an acknowledged observation start before dispatch and a defined post-dispatch observation boundary. Verify that concurrent host input remains with that receiver, requested synthetic events do not leak to it, and another target window does not act. Target counters must advance exactly once, with expected semantic state, not merely produce a log line saying a post succeeded. Capture and retain raw traces with the exact reproduction commands and environmental facts in the readiness evidence.

Negative controls must demonstrate that the observation setup detects a brief activate-and-restore, key-window change without raise, pointer move-and-restore, Space change, and diverted input; it must not misclassify the fixture's deliberate hardware input as target action. Test simultaneous target-caused interference and host input rather than treating hardware activity as a blanket waiver. Record the sampling/notification coverage and loss behavior; no finite trace establishes that target code can never act later.

Permanent regressions cover wrong process/window, duplicate AX identifiers/titles, disabled/unsupported elements, partial AX searches, stale/replaced elements, `exec`/exit/PID reuse, missing symbols/grants, invalid coordinates/chords, wrong-window key delivery, duplicate transport delivery, action timeout after a real side effect, interrupted release, and interference. Test successful actuation and expected rejection, not just parser wiring or mock forwarding.

### 6. Integration and delivery

Keep AX action work in the accessibility capability and synthetic input in a focused macOS capability boundary, depending inward on portable core contracts. Share process incarnation representation/querying where necessary without importing native injection into external interaction. The CLI owns selection, registry entries, orchestration, JSON rendering, and help; the native inspection helper remains unchanged unless a separately approved design requires otherwise. Do not add a generic scripting endpoint, plugin system, or alternative interaction executable.

Every command is registered through the existing registry with truthful mutation, target-code execution, permission-request, and no-focus effects. Observation commands stay read-only. Publish the action error schemas and update `context` with the actual non-prompting input prerequisites and required interfaces; do not describe grants or symbols as actuation support.

After the human resolves this draft and sets `awaiting-implementation`, derive behavioral tests first, then write an interaction guide from this design, then implement. Add a named native interaction task, proposed as `mise run test:interaction`, for the AppKit/Catalyst matrix and traces; keep live-Mac checks out of Linux CI rather than faking success there. Require `mise run check`, existing affected native regressions including `mise run test:focus`, the new live action task, a real CLI smoke, and independent implementation review before readiness. A passing general gate or rejected unsupported key command cannot substitute for the positive matrix.

## Compatibility

This is additive command behavior. `ui ax tree`, `ui view`, `ui windows`, `ui screenshot`, session lifecycle, and correlation retain their observation and identity contracts. There is no top-level `ax`, revived `ui tree`, or automatic promotion of an observation node into an action handle. Existing callers do not begin sending input. The issue's original examples were never shipped and need no aliases.

No protocol change, new helper attachment, or stored action session is required merely to use external actions. The proposal does not grant authority to change host security settings. New action error codes and schema metadata are additive; existing observation errors retain their meaning.

## Implications on adoption

Users must have the necessary grants for the actual Recon executable and an unlocked supported session before the uninterrupted-action guarantee applies. Homebrew's ad-hoc signature and re-granting behavior remain as decision.0005 describes; optional local Developer ID signing does not change the action contract. A command can cause target-side effects even when a timeout or later interference makes its result a failure. Callers must observe state before deciding whether to repeat it.

Private input routing increases macOS compatibility maintenance. The exact supported builds, target families, normal/research-host prerequisites, and refusal cases must be documented from the evidence. Older releases and Intel/Rosetta are not inferred supported. Removing the preview or returning to a released Recon disables the new commands; it does not undo actions already performed in an application.

## Scope

- External window-scoped AX press, single left click by element or coordinate, and single key chord, with shared target identity and explicit mechanism outcomes.
- AppKit and actual Mac Catalyst actuation, including selected-window key delivery to background secondary/non-key windows.
- Permission checks, bounded failures and release behavior, action-aware no-interference evidence, registry/schema integration, guide, and private CLI preview required for these operations.

### Out of scope

- Text insertion, scrolling, dragging, right/double clicks, held keys, sequences, macros, clipboard changes, and automatic retry. The human selected the smaller interaction scope explicitly.
- Arbitrary AX actions or attribute setters, native method invocation, injected interaction, runtime introspection, and general code evaluation.
- Activating/reordering/focusing targets, including supposedly temporary or focus-without-raise modes.
- Simulator, connected devices, Windows, Intel/Rosetta support, and general Catalyst native-view inspection. Catalyst external interaction remains in scope and does not wait on #55.
- Claims that arbitrary application callbacks cannot activate UI, or that an event-post receipt proves the user's application-level goal completed.

## Preview

- Artifact: a privately staged, installable macOS Recon executable from this branch, alongside its build identity and the AppKit and Mac Catalyst fixture applications. Preserve the normal matched-artifact layout if exercising existing native inspection regressions; interaction itself must work without a resident helper.
- Access: use a separate local preview prefix rather than replace the user's Homebrew installation. The eventual readiness report names the exact executable path, signing/grant setup, fixture paths, reproduction commands, and recorded traces. No public release or deployment is authorized by this draft.
- Exercise: run each proposed action against two distinguishable target windows while a separate foreground receiver retains input; observe the target effects and traces. Also exercise an explicitly unsupported scope and a controlled interference failure.
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

Additional input gestures or supported window states could be proposed after independent mechanism and interference evidence. They are not deferred pieces of the selected click/key scope. A process-directed key command could be a distinct future capability, but cannot satisfy this proposal's required selected-window outcome.

## Alternatives considered

- **AX press only.** A useful smaller feature for controls that expose semantic actions, but excludes the issue's synthetic click and shortcut workflow. The human chose the external three-operation scope instead.
- **Process-directed keyboard first.** Matches the known winctl key mechanism and could honestly reject `--window`. The human rejected it as sufficient: selected-window key delivery is required now, and uncertainty remains a blocker rather than a scope reduction.
- **Broader input suite.** Text, scroll, drag, and holds each add routing, interpretation, and cleanup semantics. The human selected clicks and key chords rather than that expanded proposal.
- **Copy winctl wholesale.** Its private mouse routing is useful prior art, but its target selection, PID-only keyboard path, dual-post mouse path, research-host assumptions, and endpoint-only pointer comparison do not establish Recon's contract.
- **Activate without raise, then restore, or use global input.** Changes the user's focus/input even if the desktop looks identical later. Violates policy.0004 rather than providing a fallback.
- **Require an injected helper for every action.** Excludes supported external use, adds unrelated access prerequisites, and conflicts with #24's separation from native inspection. Not chosen.

## Open questions

- [NEEDS CLARIFICATION: Which external mechanism actually delivers a chord to the selected background secondary/non-key window in both AppKit and Mac Catalyst without key-window/system-focus changes? Record a reproducible probe, target-side semantic effects, independent interference traces, exact host/security/permission conditions, target-family detection, incarnation-binding/race limits, and the resulting supported-build matrix before approval. PID-only posting or a rejection-only implementation does not answer this question.]
- [NEEDS CLARIFICATION: What concrete action-aware guard and trace design distinguishes intended target-directed events from diverted host input, observes transient key/focus/Space changes, and preserves attribution during concurrent hardware input? Demonstrate its positive and negative controls, startup/observation boundaries, loss handling, permissions, and limitations before treating no-interference as established.]
- [NEEDS CLARIFICATION: Settle the key-name/layout contract and complete action response/error schemas, numeric lookup/dispatch/release deadlines, and partial-release semantics from the selected mechanisms. The draft's operation and safety requirements are fixed, but these remaining contract choices must be precise before tests and implementation can derive from them.]

## Acknowledgments

winctl's published hit-testing experiments distinguish delivery from actuation and provide the private mouse-routing starting point. Apple's SDK contracts establish the public API and timeout boundaries. Mark selected the scope and required window-directed keyboard delivery; the unresolved research must meet that requirement rather than redefine it.
