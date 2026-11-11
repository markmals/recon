---
id: proposal.0007
title: macOS UI Interaction
authors: [Mark Malstrom, OpenAI Codex]
status: active-review
pull-request: https://github.com/markmals/recon/pull/91
issues: ["https://github.com/markmals/recon/issues/24"]
supersedes: []
---

# macOS UI Interaction

## Summary

Add window-targeted clicks, scrolling, and key chords to Recon's `ui` surface without changing the user's frontmost app, window order, Space, pointer, or input. One click command chooses AX or mouse delivery before acting. AppKit key chords are external; Mac Catalyst key chords use an explicitly opened session.

## Motivation

Recon can inspect an application's interface but cannot perform the next step in an investigation: press an observed control or invoke a shortcut and observe what changes. An agent should be able to investigate a background application while the person using the Mac keeps working elsewhere. Bringing the target forward, operating it, and restoring the desktop does not satisfy that goal: the interruption itself is the failure.

Issue [#24](https://github.com/markmals/recon/issues/24) identifies winctl as prior art and separates explicit actions from the completed observation work in #15. Mac Catalyst is a required target family, not an inference from an AppKit demonstration.

The owner selected a unified click command with automatic mechanism selection and an explicit override; left, right, middle, double, and modifier clicks; horizontal and vertical scrolling; and key chords delivered to a selected window, including a background application's non-key window. Live feasibility spikes on 2026-11-07 then showed that hover and drag-and-drop cannot avoid interrupting the user with the mechanisms found, so the owner removed them from this proposal. The same spikes showed that Catalyst key chords need code running inside the target, and the owner chose a session-backed route: Recon's helper makes the selected window key inside UIKit, then Recon sends a real key press. Trackpad pinch, rotation, and swipe remain in [#92](https://github.com/markmals/recon/issues/92).

## Domain grounding

### Established context

- Apple's [AXUIElementPerformAction](https://developer.apple.com/documentation/applicationservices/1462091-axuielementperformaction) requests an action on an accessibility object. The installed Xcode 27.1 `HIServices/AXUIElement.h`, lines 314–331, says `kAXErrorCannotComplete` can occur when an action callback runs modal work past the messaging timeout and does not mean the action failed. Retrying a mutating operation can duplicate its effects.
- Apple's [CGEvent.postToPid](<https://developer.apple.com/documentation/coregraphics/cgevent/posttopid(_:)>) addresses a process, not a window, and returns no delivery acknowledgment. The same header separates non-prompting `CGPreflightPostEventAccess` from potentially prompting `CGRequestPostEventAccess`.
- Apple's [CGEvent API](https://developer.apple.com/documentation/coregraphics/cgevent) builds mouse events by button type and scroll-wheel events in line or pixel units. [NSEvent.EventType](https://developer.apple.com/documentation/appkit/nsevent/eventtype) distinguishes button, motion, and wheel events from trackpad magnify, rotate, and swipe events.
- [NSApplication.sendEvent](<https://developer.apple.com/documentation/appkit/nsapplication/sendevent(_:)>) forwards key events to the application's key window for dispatch.
- [winctl's pinned hit-testing experiment](https://github.com/NSExceptional/winctl/blob/295a02751b2cb0c958b476e6ce6864963ee4042f/research/hittest/README.md) records a background AppKit button firing after private routing field 51 and mouse fields were set on a process-addressed event, on macOS 15.7 with relaxed security. It records synthetic delivery stopping at a password-locked screen. [winctl's implementation](https://github.com/NSExceptional/winctl/blob/295a02751b2cb0c958b476e6ce6864963ee4042f/winctl.m) posts keys to the process only and posts each mouse event through two transports.
- The [cua driver](https://github.com/trycua/cua/blob/1759af24ec84685269529425ac42e5de964adf85/libs/cua-driver/rust/crates/platform-macos/src/input/skylight.rs) sends key-window records to a single process through `SLPSPostEventRecordTo` to change that process's key window without fronting it. This is source observation; Recon's own evidence follows.

### Spike evidence

Recorded on Apple silicon, macOS 27.0.1 (26A434), Xcode 27.1 (27A9269), on the owner's research Mac (SIP disabled, AMFI relaxed). A separate signed controller with its own privacy identity posted input to two instrumented fixtures, each with two independently instrumented windows: an AppKit app and a real Mac Catalyst app (`LC_BUILD_VERSION` platform `MACCATALYST`). Throughout every run the controller sampled the frontmost application, the on-screen window order, the active Space, the real pointer, and hardware and session input counters, and observed workspace activation and Space notifications. Targets were background windows, mostly covered by another application. Success meant the fixture's own handler acted in the selected window.

- **Mouse clicks.** `CGEventPostToPid` events carrying the target window number (field 51), the target PID (field 40), mouse fields 1, 3, 7, and 58, and a window-local location (`CGEventSetWindowLocation`) actuated the selected covered window: AppKit left, right, middle, double, and Shift clicks, and a Catalyst left-click on a button. `SLEventPostToPid` behaved the same. No pointer movement, frontmost change, or window reorder occurred. An AppKit left click on a non-key window made it the key window inside the background target.
- **Scrolling.** A wheel event built the same way scrolled the AppKit outer and nested scroll views and the Catalyst scroll view under the addressed point. A negative wheel value advanced content down or right.
- **AppKit keys.** AppKit sends key events to its own key window and ignores the addressed window: a chord addressed to one window ran the other window's action through the public route, the private route, and the authenticated private route. After target-only key-window records sent through `SLPSPostEventRecordTo` to the target's process serial number, the selected window became key inside the target, the target stayed in the background, window order was unchanged, and subsequent chords ran only the selected window's action, in both directions. Inside the target, AppKit then reports itself active although it is not frontmost.
- **Catalyst keys.** The external key event reached the selected window's AppKit bridge view, but UIKit dispatches keys to its own application key window, which stayed unset; no key press or key command reached UIKit. Setting `AXMain` on the window reordered windows and still left UIKit without a key window. With Recon's existing standard attach session the helper loaded into the cooperative Catalyst fixture and `ui view tree` read its AppKit layer. Code running inside the fixture that made the selected `UIWindow` key changed only UIKit's key window, with no frontmost or order change. A real chord then ran only that window's key command.
- **Held-key hazard.** A key-up carrying the Command flag never reached UIKit, so UIKit treated B as held and repeated the command about six times per second, 248 times before release. Sending the chord as a keyboard does — Command down, key down, Command up, key up — produced exactly one action with balanced press begin and end.
- **Counters.** Process-addressed key posts left the combined-session key counter unchanged, so the existing diverted-input check still detects session-level synthetic input.
- **Hover (removed).** Process-addressed mouse-moved events arrived at the right AppKit window location, singly or as an entering path, but no tracking area or hover recognizer changed; Catalyst received none.
- **Drag-and-drop (removed).** The press-and-drag started a genuine AppKit drag session, but the system chose the drop recipient as the topmost window at the drop location: the covering, unrelated application received the drop and requested the data.

### Relevant constraints and principles

A bundle, a process incarnation, a WindowServer window, an AX element, and a native view are different things. A bundle can have several instances, a process several windows, and an AX title or identifier is not necessarily unique. A historical tree path is an observation location, not an action handle. A PID can name a new incarnation after exit and reuse, or after `exec`.

AX press is a semantic action; a mouse click is a button gesture at a point. Both satisfy a plain element click, so they share one command, but they remain distinguishable mechanisms in its override and result. Right, middle, double, and modifier clicks require mouse semantics.

System keyboard focus and an application's own key window are different. The user's typing goes to the frontmost application's key window. A background application's key window decides only where that application sends key events delivered to it. Changing the latter while the application stays in the background does not move the user's input.

Selecting a window, delivering an event, invoking a callback, and achieving the caller's application-level goal are separate facts. Target callbacks can activate UI or keep running after Recon stops waiting; Recon cannot make target code pure or undo it.

### Quality bar

Every supported action must show the intended effect in the selected window and no change to the frontmost application, window order, Space, real pointer, or the user's input across the operation. A successful API return, a delivered event without an effect, or an AppKit run labelled Catalyst is insufficient. Unsupported requests are rejected before input. Unknown execution after a timeout stays unknown and is never retried automatically.

### Remaining uncertainty

The spikes did not exercise AX press on either toolkit, or Catalyst right, middle, double, and modifier clicks. They measured counter behaviour for posted key events but not for posted button and wheel events. They proved the physical chord sequence on Catalyst but not on AppKit. They proved Catalyst key-window change from code inside the fixture, not from the helper, which must still map a WindowServer window to its `UIWindow`. They ran only with relaxed host security. Implementation must produce this evidence; a required row that fails returns the proposal to the owner rather than narrowing scope silently.

## Existing baseline

Recon already supplies strict bundle, path, and name selection, instance disambiguation, window ownership checks, optional session identity assertions, executable-owned privacy identity, typed errors, and registry-derived help and schemas (`crates/recon-applications-macos/src/selection.rs`, `crates/recon-window-server-macos/src/window.rs`, `crates/recon/src/observe/target.rs`). Interaction reuses them rather than winctl's substring and largest-window selection.

The accessibility crate reads role, title, identifier, enabled state, frame, and advertised actions, with a two-second messaging timeout. External selection returns a PID without binding an incarnation; the native session path binds `Incarnation { pid, pidversion }`.

`crates/recon-ui-guard-macos/src/focus.rs` samples the frontmost process, window order, Space, pointer, and hardware and session counters every ten milliseconds. Its window-order evidence is not a key-window observation, snapshots can miss changes shorter than one interval, and any interval with hardware input is attributed to the user.

The native session loader and helper attach to cooperative targets under the standard profile and to protected targets on a prepared research host. The helper exposes closed inspection operations. Recon's fixture is AppKit only; there is no Catalyst fixture or live interaction task.

## Proposed solution

```sh
recon ui click "Notes" --window 123 --role AXButton --title Share
recon ui click "Notes" --window 123 --role AXButton --title Share --method mouse
recon ui click "Notes" --window 123 --x 160 --y 84 --button right
recon ui click "Notes" --window 123 --x 160 --y 84 --count 2 --modifiers shift
recon ui scroll "Notes" --window 123 --x 160 --y 284 --dy 300
recon ui key "TextEdit" cmd+b --window 456
recon ui key "Home" cmd+f --window 789 --session 0123456789abcdef0123456789abcdef
```

Clicks and scrolling address the selected window directly and need no session. A key chord first makes the selected window key inside the background target, then sends a real key press shaped like a physical one. AppKit targets need no session; Catalyst targets require an open session whose helper makes the window key inside UIKit. Every operation runs under an interaction-aware guard that fails it if it interrupts the user, and reports the mechanism it used. Hover and drag-and-drop are not offered.

## Detailed design

### 1. Targets and identity

Every operation selects a running application through the existing target grammar, with `--pid` to disambiguate instances and a required `--window` that must belong to that instance. No operation infers a window or launches an application. Existing selection failures keep their codes and send no input.

Before resolving the action Recon binds the process incarnation — PID and audit-token `pidversion` — and the window. It rechecks both immediately before each posted event and before any release event. An exit, `exec`, owner change, or missing window stops the operation with `STALE_TARGET`; Recon never re-resolves and continues. The residual race between the last check and a process-addressed post (the process exiting and its PID being reused within that interval) is accepted and documented.

`--session` remains an identity assertion: a stale or mismatched session fails with the existing session errors and is never reopened or substituted. Only Catalyst key chords use a session to run code; every other operation ignores the session beyond the assertion.

Synthetic input requires an unlocked console session. A locked screen, reported by the current session dictionary, makes every operation fail with `ACTION_UNAVAILABLE` before input. Secure event input (`IsSecureEventInputEnabled`) makes `ui key` fail the same way.

The toolkit of the target is read from the `LC_BUILD_VERSION` platform of its running executable: platform `MACCATALYST` is Catalyst, `MACOS` is AppKit. An unreadable platform fails `ui key` with `ACTION_UNAVAILABLE`.

### 2. Locations and element resolution

`ui click` and `ui scroll` take either one or more of `--identifier`, `--role`, and `--title`, or a complete `--x`/`--y` pair, never both. Filters combine with AND and match the published AX fields exactly and case-sensitively. Roles use AX strings such as `AXButton`.

Resolution walks a fresh AX tree of the selected window with the existing depth bound of 64 and two-second messaging timeout, under an overall five-second lookup deadline. Zero matches is `NOT_FOUND`; two or more is `AMBIGUOUS_ELEMENT`; a traversal with any omitted, failed, or timed-out subtree cannot establish uniqueness and fails with `TIMEOUT` (for timeouts) or `ACTION_UNAVAILABLE`. A lookup failure occurs before any input, so `TIMEOUT` keeps its existing retryable meaning. The live element is revalidated — same filters, same window — immediately before dispatch.

Coordinates are finite logical points from the top-left of the WindowServer window bounds, frame included, with `0 <= x < width` and `0 <= y < height`. Element locations use the centre of a finite, non-empty frame that lies inside the window. Bounds are read fresh; Recon never clamps or moves the window. An element click requires a known enabled state; disabled or unknown is `ACTION_UNAVAILABLE`.

### 3. Click

`ui click` accepts `--method auto|ax|mouse` (default `auto`), `--button left|right|middle` (default `left`), `--count 1|2` (default `1`), and `--modifiers`, a comma-separated set of `cmd`, `ctrl`, `alt`, `shift` with no duplicates.

| Request | Method |
| --- | --- |
| `auto`, element, left, count 1, no modifiers | AX press when the element advertises `AXPress`; otherwise mouse at the element centre |
| `auto` with coordinates, right or middle, count 2, or modifiers | Mouse |
| `ax` | Requires element, left, count 1, no modifiers, and advertised `AXPress`; contradictory options are `INVALID_ARGUMENT`; no `AXPress` is `ACTION_UNAVAILABLE` |
| `mouse` | Mouse, even when `AXPress` is advertised |

The method is chosen before any mutating call. A capability read failure, permission denial, or incomplete search never selects the other method, and no attempted method is ever followed by the other.

AX delivery invokes `AXPress` once under the two-second messaging timeout. `kAXErrorCannotComplete` is `ACTION_TIMEOUT` with unknown execution.

Mouse delivery creates events at the global point (window origin plus local point) and posts them with `CGEventPostToPid` only. Each event carries field 51 = window number, field 40 = PID, field 1 = click state, field 3 = button number, field 7 = 3, field 58 = a per-request click group, and the window-local location set with `CGEventSetWindowLocation`; fields are set with `SLEventSetIntegerValueField`, resolved at runtime. Missing private symbols are `INTERFACE_UNAVAILABLE`. Each click is a down and an up 30 ms apart; a double click sends a second pair 60 ms later with click state 2. Modifiers are carried in the event flags only; no modifier key events are posted, so no modifier can be left held. Right and middle clicks use the secondary and other button event types, never Control-click or `AXShowMenu`.

When the target is the frontmost application and the selected window is not its key window, a mouse click is `ACTION_UNAVAILABLE`: it would move the user's keyboard focus. AX press and scrolling remain available there. When the target is in the background, a click may make the selected window key inside the target; the result reports it.

Mac Catalyst limits what a background window receives. Its first click there reaches UIKit only over a control; over any other view, the click is taken as window activation and never reaches the application, and UIKit offers no way to accept it. Middle-button events never reach UIKit at all. Recon sends these clicks the same way and reports them `unverified` like every click; it neither detects nor refuses them.

### 4. Scroll

`ui scroll` takes signed integer `--dx` and `--dy` (omitted is zero, at least one non-zero) and `--unit pixel|line` (default `pixel`). Positive values advance content right or down, independent of the natural-scrolling preference. Each magnitude is at most 10000 pixels or 1000 lines.

Recon posts one wheel event created with `CGEventCreateScrollWheelEvent2(unit, 2, -dy, -dx, 0)`, located at the global point and carrying field 40, field 51, and the window-local location, with `CGEventPostToPid`. It sets no phase or momentum fields and adds no inertia. Units describe input, not guaranteed displacement; an element selects only the event location.

### 5. Key chords

#### Grammar

`ui key TARGET CHORD --window ID` takes one chord: `+`-separated, case-insensitive tokens with no whitespace, zero or more distinct modifiers, and exactly one base key. Modifiers are `cmd` (alias `command`), `ctrl` (`control`), `alt` (`option`, `opt`), and `shift`. Base keys are `a`–`z`, `0`–`9`, `` ` `` `-` `=` `[` `]` `\` `;` `'` `,` `.` `/`, and the named keys `return`, `tab`, `space`, `escape` (`esc`), `delete`, `forward-delete`, `left`, `right`, `up`, `down`, `home`, `end`, `pageup`, `pagedown`, `f1`–`f12`. Anything else, a duplicate modifier, a modifier-only chord, or more than one base key is `INVALID_ARGUMENT` before any input. An uppercase letter means the same key; Shift is never implied.

Character keys resolve through the current keyboard input source's layout: Recon finds the virtual key code that types that character without modifiers. A character no key produces is `ACTION_UNAVAILABLE`. Named keys use fixed virtual key codes.

#### Making the selected window key inside the target

Recon reads the target's key window as its accessibility `AXFocusedWindow`, mapped to a window number with `_AXUIElementGetWindow`.

- If the target is the frontmost application, the chord is delivered only when the selected window is already its key window; otherwise `ACTION_UNAVAILABLE`, because changing it would move the user's input.
- If the selected window is already the target's key window, nothing changes.
- Otherwise, for an AppKit target, Recon sends records only to the target's process serial number, resolved with `SLSGetWindowOwner` and `SLSGetConnectionPSN`, through `SLPSPostEventRecordTo`: a defocus record for the current key window when one is known, a focus record for the selected window, and the make-key record pair, 120 ms apart. Nothing is sent to the frontmost or any other process. Recon then waits up to one second, polling every 40 ms, for `AXFocusedWindow` to report the selected window.
- For a Catalyst target, `--session` naming an open session for the same incarnation is required; without it the chord fails with `ACTION_UNAVAILABLE` naming the session requirement, and no session is opened automatically. Recon asks the helper's interaction operation to make key the `UIWindow` hosted by the selected window number. The helper runs on the target's main thread, resolves exactly one `UIWindow` for that window number or fails, calls `makeKey()`, and returns the previous and new UIKit key windows. Recon requires the reported new key window to be the selected one.

A failed or unconfirmed key-window change is `ACTION_FAILED` at stage `key-window` with no key events sent; any key-window change already made is reported, not reversed. Recon never restores a previous key window afterwards: that would be a second change.

#### Sending the chord

The chord is sent like a physical press, with `CGEventPostToPid` events carrying field 40 and field 51: each modifier key down in the order `cmd`, `ctrl`, `alt`, `shift` with cumulative flags; the base key down with all flags; each modifier key up in reverse order with the remaining flags; the base key up with no flags. Consecutive events are 30 ms apart. A chord addressed to a window that is not the target's key window is never sent.

### 6. Interaction-aware guard

Every operation runs inside the shared guard with an `interaction` stage and the bound target. The guard starts with a readable baseline before the first change or event and keeps watching for 300 ms after the last event or AX return.

- **Kept dimensions.** Frontmost process, on-screen window order, active Space, real pointer location and hardware pointer counters, and the diverted-input check (session input counted beyond hardware input). Process-addressed posts do not move session counters, so any session-level synthetic input still fails the operation; implementation must show this holds for button and wheel events as well as keys.
- **Transient changes.** The guard also observes workspace application-activation and active-Space notifications for the whole interval on its own run-loop thread, so a change undone between samples is still caught. If the observers cannot be registered the operation fails with `INTERFACE_UNAVAILABLE` before any input.
- **The user's key window.** At the start and end of the interval, the guard reads the frontmost application's `AXFocusedWindow`; a change not explained by hardware input is interference.
- **Target-local changes.** A change of the target's own key window, main window, or internal active state while the target remains in the background is expected and reported in the result. Any target window coming in front of a window it was behind is still interference.
- **Attribution.** During an interaction, a frontmost change to the target, or a target window moving forward in the window order, is attributed to the target even when hardware input occurred in the same interval, because the operation posted the input that could have caused it. Other changes during hardware input remain the user's. A read posts nothing, so the shared guard applies this target-wins rule only to the interaction stage: for a read, a target frontmost change or window-order move that coincides with hardware input is the person's own action and does not fail the read, while the same move with no hardware input remains interference in either stage.

Interference returns `UI_INTERFERENCE` with stage `interaction`, evidence source, affected process and window, and whether target execution may continue. Recon stops unsent events, sends only the releases below, and never restores focus, order, pointer, or Space.

No event tap is used, so event-listening permission is not required.

### 7. Release, results, and errors

If an operation stops after a down event — failure, interference, or a changed identity — Recon sends only the matching releases still owed, in the specified order, through the same route to the same incarnation: the button up for a click, and for a chord the remaining modifier ups and the base key up with no flags. It sends none when the identity changed. The result or error reports `release` as `not-needed`, `complete`, `failed`, or `skipped-target-changed`.

A successful response contains `application` (`path`, `pid`, `pidversion`), `window`, `operation` (`click`, `scroll`, or `key`), `mechanism` (`ax-press`, `mouse-events`, `scroll-wheel`, or `key-events`), and `outcome: "unverified"`, because Recon does not observe the application-level result. Click adds `method` (`requested`, `effective`, `reason`), `button`, `count`, `modifiers`, the window-local `location`, and the matched `element` (role, title, identifier) when a selector was used. Scroll adds `delta` (`x`, `y`, `unit`). Key adds `chord` (`modifiers`, `key`) and `keyWindow` (`mechanism`: `already-key`, `window-server-records`, or `uikit-helper`; `before`; `after`). Every response reports `targetKeyWindowChanged` when the target's own key window changed.

New schema-published error codes, none retryable:

| Code | Meaning |
| --- | --- |
| `ACTION_UNAVAILABLE` | The requested action, scope, or prerequisite cannot be honoured without breaking the contract; nothing was sent |
| `AMBIGUOUS_ELEMENT` | More than one element matched the selector |
| `STALE_TARGET` | The bound process incarnation or window changed |
| `ACTION_FAILED` | A mechanism reported failure at a named stage |
| `ACTION_TIMEOUT` | A mutating call did not finish in time; execution is unknown |

Action error details contain `stage` (`resolve`, `key-window`, `dispatch`, `release`), `dispatch` (`none`, `partial`, `complete`, `unknown`), `mechanism`, `pid`, `windowId`, and `release`. Pre-dispatch AX timeouts keep `TIMEOUT`; selection, permission, interface, session, and interference failures keep their existing codes. Nothing is retried automatically. The whole operation, including key-window confirmation, has a ten-second deadline; exceeding it after any change is `ACTION_TIMEOUT`.

### 8. Permissions, registry, and helper

AX lookup, AX press, and `AXFocusedWindow` reads require Accessibility; posting requires event-posting access, checked with `CGPreflightPostEventAccess`. A coordinate-only click or scroll needs no Accessibility unless the target's key-window or frontmost state must be read. A missing grant may be requested under the existing setup exception and then fails with `PERMISSION_DENIED` before input. `context` and preflight stay non-prompting and report event-posting state alongside the existing grants.

`ui click`, `ui scroll`, and `ui key` are registered through the existing registry with effects `sends-input` and `changes-target-key-window`; `ui key` on a Catalyst target additionally declares `executes-target-code`. Each declares the guard errors and the action errors above.

The helper gains one closed interaction operation, make-window-key, in a Catalyst interaction extension built with the Swift extension SDK. The helper remains a macOS library and reaches UIKit through the Objective-C runtime. The operation is reachable only from `ui key` on a Catalyst target with an open session; `ui view` operations stay read-only. `ui session preflight` and `open` report a Catalyst target's toolkit and accept it for this purpose; native UIKit view inspection remains #55.

Input routing lives in a focused macOS input capability crate; AX press lives in the accessibility crate; the CLI owns selection, orchestration, and output.

### 9. Required evidence

The initial host boundary is native Apple silicon on macOS 27 with an unlocked console session. Evidence records host build, target versions, architecture, signing, grants, and SIP and AMFI state; evidence obtained only with relaxed security does not establish stock-host support.

| Operation | AppKit | Mac Catalyst |
| --- | --- | --- |
| Click | AX press and forced mouse on an element; coordinate left, right, middle, double, and modifier clicks each produce their own handler effect once | AX press and forced mouse on an element; coordinate left, right, double, and modifier clicks over a control each produce their own handler effect once, in a real Catalyst scene |
| Scroll | Both axes, both directions, both units, outer and nested regions | The same |
| Key chord | The selected window's action runs, for each of two windows, from a background target, with no repeated action | The same, through a session, with balanced press begin and end |

Each row covers a background target, a non-key window, and a covered window on the current Space, with a second window in the same process whose effects must not change, while a separate foreground receiver keeps the user's input. Results show no frontmost change, window reorder, Space change, pointer motion, or diverted input. A frontmost target covers the frontmost refusal rules.

Negative controls show that the guard reports an activate-and-restore, an AX raise of a target window, a pointer warp-and-restore, a Space switch, a session-level synthetic keystroke, and a key-window change in the frontmost application; and that deliberate hardware input during an unrelated change is not blamed on the target.

Permanent regressions cover wrong process or window, duplicate identifiers or titles, disabled elements, incomplete searches, `exec`, exit, and PID reuse, missing symbols and grants, invalid coordinates and chords, method selection and refusal, no retry after an AX timeout, frontmost refusals, Catalyst without a session, unconfirmed key-window change, release after a mid-chord failure, and the held-key hazard. Tests assert effects and refusals, not parser wiring.

A live task, `mise run test:interaction`, builds the AppKit and Catalyst fixtures and the foreground receiver and runs this matrix. Like the other native tasks, it stays out of `mise run check`.

## Compatibility

Additive. `ui ax tree`, `ui view`, `ui windows`, `ui screenshot`, session lifecycle, and correlation keep their contracts, and `ui view` stays read-only. The helper protocol adds one operation; the protocol version changes, and a resident helper from an earlier build fails with the existing protocol-mismatch error. New error codes and effects are additive.

## Implications on adoption

Interaction needs Accessibility and event-posting grants for the installed Recon executable and an unlocked session. Homebrew's ad-hoc signature still requires re-granting after upgrades. Catalyst key chords additionally need a session, which loads Recon's helper into the target: cooperative targets under the standard profile, protected targets only on a prepared research host.

The routes depend on private WindowServer interfaces and field numbers; each macOS update requires re-running the live evidence. Older macOS, Intel, and Rosetta are not inferred supported. A command's effects in an application persist even when it reports failure.

## Scope

- `ui click` with automatic or forced AX or mouse delivery, left, right, and middle buttons, single and double clicks, and modifiers.
- `ui scroll` with horizontal and vertical wheel input in pixels or lines.
- `ui key` for AppKit targets externally and Catalyst targets through a session, including the helper's make-window-key operation and Catalyst session acceptance.
- The interaction-aware guard, releases, errors, schemas, `context` reporting, a guide, AppKit and Catalyst fixtures, the live evidence task, and a private preview.

### Out of scope

- Hover and drag-and-drop: the spikes found no mechanism that avoids interrupting the user.
- Trackpad pinch, rotation, and swipe ([#92](https://github.com/markmals/recon/issues/92)).
- Text insertion, triple clicks, held keys, key sequences, macros, and automatic retry.
- Arbitrary AX actions or attribute writes, native method calls, and general code evaluation.
- Activating or raising targets, focus-without-raise, and restoring focus.
- UIKit view inspection (#55), Simulator, devices, Windows, Intel, and Rosetta.

## Preview

- Artifact: a privately staged, installable Recon executable from this branch with its loader, helper, and build identity, plus the AppKit and Catalyst fixtures and the foreground receiver.
- Access: a separate local prefix, not the Homebrew installation. The readiness report names the executable path, signing and grant setup, fixture paths, commands, and recorded traces. No public release.
- Exercise: each operation and click override against two windows per fixture while the foreground receiver keeps input; AppKit key chords externally and Catalyst key chords through a session; a frontmost refusal; Catalyst without a session; and a controlled interference failure.
- Cleanup: stop only preview-owned processes and sessions and remove only preview-owned files.
- Reason: an executable and instrumented targets are the cheapest way to see real actuation and the absence of interference.

## Policies and decisions checked

- `VISION.md` — explicit effects, evidence provenance, and honest refusals.
- [policy.0001](../policies/0001-cli-command-and-error-contract.md) — commands through the registry and every failure with a published code.
- [policy.0005](../policies/0005-ui-no-interruption.md) — no `ui` operation may interrupt what the user is doing. It supersedes policy.0004, whose list of forbidden mechanisms included making any window key; changes inside a background target that the user does not experience, such as its own key window, are outside the rule. The guard in section 6 is how this proposal meets it, and its live evidence is the proof.
- [decision.0006](../decisions/0006-native-helper-may-manipulate-targets.md) — the helper may manipulate the target as well as inspect it. It supersedes and withdraws decision.0003, so the Catalyst make-window-key operation needs no exception.
- [decision.0005](../decisions/0005-homebrew-ad-hoc-privacy-identity.md) and [decision.0002](../decisions/0002-macos-context-privacy-identity.md) — executable-owned identity, the setup-prompt exception, and non-prompting `context`.
- [decision.0004](../decisions/0004-homebrew-bottle-releases.md) — no new distribution channel; the helper still ships beside the CLI.
- [proposal.0005](0005-native-macos-inspection.md#5-shared-ui-integration-no-focus-behavior-and-ax-correlation) — fresh target and window validation, sessions as identity assertions, and a posted event distinguished from actuation. Catalyst key chords are the owner-approved exception to "no mandatory injection for external actions".

## Future directions

- Hover and drag-and-drop could return if a mechanism is found that delivers tracking or drops to a selected window without the real pointer or the topmost window.
- Catalyst key chords without a session would need an external way to set UIKit's key window.
- Trackpad gestures remain in #92.

## Alternatives considered

- **Separate AX press and mouse click commands.** Two interfaces for one intent; replaced by one command with a pre-dispatch rule and override.
- **Process-directed keys.** AppKit routes keys to its own key window, so the selected window is ignored; rejected by the owner as insufficient.
- **Activate without raise, or activate and restore.** Moves the user's input; forbidden.
- **Accessibility `AXMain` or `AXFocused` writes.** `AXMain` reordered windows in the spike and did not set UIKit's key window.
- **Invoking a Catalyst key command through the responder chain.** Worked in the spike without input, but covers only key commands; the owner chose a real key press.
- **Sending keys with Command held through the key-up.** Leaves UIKit repeating the command; replaced by the physical sequence.
- **Dropping Catalyst keys.** Considered; the owner chose the session-backed route.
- **Restricting drag to visible destinations.** Racy and still passes over other windows; drag-and-drop was removed instead.
- **Requiring a helper for every action.** Clicks and scrolling work externally; only Catalyst keys need one.

## Open questions

None.

## Acknowledgments

winctl's hit-testing research and the cua driver's key-window records supplied the starting points the spikes verified. Mark selected the scope and the session-backed Catalyst route.
