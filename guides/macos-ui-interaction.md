---
id: guide.macos-ui-interaction
title: Interact with macOS Application UI
describes: [proposal.0007]
---

# Interact with macOS Application UI

## What it does

Recon can click or scroll a selected window and send one key chord to a running macOS application. The commands are `recon ui click`, `recon ui scroll`, and `recon ui key`. They address a WindowServer window that belongs to the selected application instance; they never infer a window, launch the application, or reuse a historical accessibility path as an action handle.

Interaction changes the target application, not the user's desktop. Recon watches the frontmost application, window order, active Space, real pointer, keyboard focus, and input counters throughout the operation. An operation that disturbs the user's work fails with `UI_INTERFERENCE`; Recon does not activate and restore an app, raise and lower a window, warp the pointer back, switch Spaces back, or retry the action automatically. A successful result reports the delivery mechanism and `outcome: "unverified"`: delivery is not proof that the application achieved the caller's larger goal.

Hover, drag-and-drop, trackpad gestures, text insertion, held keys, key sequences, macros, arbitrary accessibility actions, and automatic retries are not provided.

## How to use it

### Check the host, target, and grants

Interaction supports native Apple silicon on macOS 27 with an unlocked console session. A locked screen makes every action fail with `ACTION_UNAVAILABLE` before input. Secure event input also makes `ui key` fail with `ACTION_UNAVAILABLE`.

Run `recon context` before acting. It reports Recon's Accessibility and event-posting grants and the private interfaces these commands require without requesting a grant. Element lookup, AX press, and key-window reads need Accessibility; mouse, wheel, and key delivery need event-posting access. A coordinate-only click or scroll does not otherwise read the accessibility tree. If an action finds a required grant missing, it may request macOS authorization as a setup step and then fails with `PERMISSION_DENIED` before sending input. Grants belong to the installed `recon` executable; see [Observe macOS Application UI](macos-ui-observation.md) for signing and upgrade implications.

Choose the target with the same exact application name, bundle identifier, or absolute `.app` path used by other `ui` commands. Add `--pid PID` when the bundle has multiple running instances. Every action requires `--window ID`, using a current ID from `recon ui windows TARGET`. Recon binds the process incarnation and window before acting and checks them again before every event and release. Exit, `exec`, window destruction, owner change, or PID reuse stops the operation with `STALE_TARGET`; Recon never resolves a replacement and continues.

An optional `--session SESSION` is an identity assertion. It must name an open session for the same application incarnation and is never reopened or substituted. Only a Mac Catalyst key chord uses that session to execute target code; clicks, scrolling, and AppKit key chords remain external.

### Click an element or point

Use exact, case-sensitive accessibility fields to select an element, or use a window-local point:

```sh
recon ui click "$APP" --window "$WINDOW" --role AXButton --title Share
recon ui click "$APP" --window "$WINDOW" --x 160 --y 84
```

`--identifier`, `--role`, and `--title` may be combined and are ANDed. At least one filter is required for an element click. Recon walks a fresh accessibility tree of the selected window, to depth 64, with a five-second overall lookup deadline and two-second accessibility messaging timeout. Zero matches returns `NOT_FOUND`; multiple matches returns `AMBIGUOUS_ELEMENT`. An omitted, failed, or timed-out subtree cannot prove uniqueness, so the lookup fails instead of clicking a partial match. Recon revalidates the live element immediately before dispatch. An element must have a known enabled state and a finite, non-empty frame inside the window; disabled or unknown state is `ACTION_UNAVAILABLE`.

Coordinates are logical points from the top-left of the WindowServer window bounds, including its frame. Both coordinates are required, must be finite, and must satisfy `0 <= x < width` and `0 <= y < height`. Recon reads fresh bounds and never clamps a point or moves a window.

`--method auto|ax|mouse` defaults to `auto`. For a plain left single-click on an element with no modifiers, auto uses one AX press when the element advertises `AXPress`; otherwise it uses window-addressed mouse events at the element centre. Coordinates, right or middle buttons, double-clicks, and modifiers always use mouse events. `--method mouse` forces mouse delivery. `--method ax` requires an eligible plain element click and advertised `AXPress`; contradictory options are `INVALID_ARGUMENT`, and an element without `AXPress` is `ACTION_UNAVAILABLE`. Recon chooses before acting and never tries the other mechanism afterward.

`--button left|right|middle` defaults to `left`. `--count 1|2` defaults to `1`. `--modifiers` is a comma-separated set containing `cmd`, `ctrl`, `alt`, and `shift`, without duplicates; modifiers affect only the click events and are not sent as held keyboard keys. A double-click sends two down/up pairs with click states 1 and 2.

When the target is already frontmost, a mouse click is available only in its current key window; clicking another window would move the user's keyboard focus and returns `ACTION_UNAVAILABLE`. AX press and scrolling remain available. A background click may change the target's own key window; the result reports that target-local change.

### Scroll an element or point

```sh
recon ui scroll "$APP" --window "$WINDOW" --x 160 --y 284 --dy 300
recon ui scroll "$APP" --window "$WINDOW" --title Content --dx -5 --unit line
```

Scrolling accepts the same element-or-coordinate location rules as clicking. `--dx` and `--dy` are signed integers; an omitted axis is zero, and at least one axis must be nonzero. Positive values advance content right or down regardless of the natural-scrolling preference. Each magnitude is at most 10,000 pixels or 1,000 lines. `--unit pixel|line` defaults to `pixel` and describes the wheel input, not a guaranteed displacement. An element selector chooses only the wheel event's location. Recon sends one wheel event without phase, momentum, or added inertia.

### Send one key chord

```sh
recon ui key "$APP" cmd+b --window "$WINDOW"
recon ui key "$CATALYST_APP" cmd+f --window "$WINDOW" --session "$SESSION"
```

A chord contains zero or more distinct modifiers and exactly one base key, joined by `+` with no whitespace. Tokens are case-insensitive. Modifiers are `cmd` (`command`), `ctrl` (`control`), `alt` (`option`, `opt`), and `shift`. Base keys are `a`–`z`, `0`–`9`, `` ` ``, `-`, `=`, `[`, `]`, `\`, `;`, `'`, `,`, `.`, `/`, and `return`, `tab`, `space`, `escape` (`esc`), `delete`, `forward-delete`, `left`, `right`, `up`, `down`, `home`, `end`, `pageup`, `pagedown`, and `f1`–`f12`. An uppercase letter names the same key and does not imply Shift. Invalid, duplicate, modifier-only, and multi-key chords are `INVALID_ARGUMENT` before input.

Character keys use the current keyboard input source's layout: Recon finds the virtual key code that types that character without modifiers. A character no key produces is `ACTION_UNAVAILABLE`. Named keys use fixed virtual key codes.

Recon reads the target's `AXFocusedWindow`. A frontmost target must already have the selected window as its key window; otherwise `ui key` returns `ACTION_UNAVAILABLE` without changing the user's input. A background target's selected window becomes key inside that application only. AppKit does this through target-only WindowServer records and needs no session. Mac Catalyst requires an already-open session for the same incarnation; without one, the command returns `ACTION_UNAVAILABLE` and does not open a session automatically. The helper resolves exactly one `UIWindow` for the selected WindowServer window and calls `makeKey()` on the target's main thread. Session preflight and open report the Catalyst toolkit and accept it for this interaction, but do not add UIKit view inspection.

Recon confirms the selected key window before sending the chord. Failure or lack of confirmation is `ACTION_FAILED` at stage `key-window`, with no key events sent. A change already made is reported and is not reversed. The chord then follows physical press ordering: modifier downs in `cmd`, `ctrl`, `alt`, `shift` order, the base-key down, modifier ups in reverse order, and finally the base-key up without flags. This balanced sequence avoids leaving Catalyst commands held and repeating.

### Read results and failures

Every success contains `application` with `path`, `pid`, and `pidversion`; `window`; `operation`; `mechanism`; and `outcome: "unverified"`. Click results also report requested and effective methods, selection reason, button, count, modifiers, window-local location, and any matched element. Scroll results report the delta and unit. Key results report the normalized chord and key-window mechanism, before/after windows, and target-local key-window change. All responses report whether the target's own key window changed.

If a failure occurs after a down event, Recon sends only the matching release events still owed to the same bound incarnation. It sends no release when that identity changed. Results and errors report `release` as `not-needed`, `complete`, `failed`, or `skipped-target-changed`. Recon stops unsent events after interference and never restores desktop state.

The action-specific error codes are all non-retryable:

| Code | Meaning |
| --- | --- |
| `ACTION_UNAVAILABLE` | The action, scope, or prerequisite cannot be honoured without breaking the contract; nothing was sent. |
| `AMBIGUOUS_ELEMENT` | More than one accessibility element matched. |
| `STALE_TARGET` | The bound process incarnation or window changed. |
| `ACTION_FAILED` | A mechanism failed at a named stage. |
| `ACTION_TIMEOUT` | A mutating call or the ten-second operation deadline expired; execution may be unknown. |

Action error details include `stage`, `dispatch`, `mechanism`, `pid`, `windowId`, and `release`. A failure after a key-window transition can also contain `keyWindow` with `before`, `after`, and `changed`; `changed: null` means Recon could not establish the resulting state. A pre-dispatch accessibility lookup timeout retains the existing retryable `TIMEOUT`; selection, permission, interface, session, and interference failures retain their existing codes. An AX press that exceeds its messaging timeout is `ACTION_TIMEOUT` with unknown execution and is never retried.

## Worked example

Set `APP` to an authorized running app's exact target, choose `WINDOW` from its current `ui windows` result, and inspect Recon's grants first:

```sh
recon context
recon ui windows "$APP"
recon ui click "$APP" --window "$WINDOW" --role AXButton --title Share
recon ui scroll "$APP" --window "$WINDOW" --x 160 --y 284 --dy 300
recon ui key "$APP" cmd+b --window "$WINDOW"
```

For a Catalyst chord, open a compatible session explicitly and use the returned identifier before sending the key:

```sh
recon ui session preflight "$CATALYST_APP"
recon ui session open "$CATALYST_APP" --mode attach
recon ui key "$CATALYST_APP" cmd+f --window "$WINDOW" --session "$SESSION"
recon ui session close "$SESSION"
```

Use `--mode existing` or `--mode launch` instead when that is the access relationship you intend; there is no automatic mode or fallback. Catalyst session access can load code into the target and has the prerequisites and risks described in [Inspect Native macOS UI](native-inspection.md).

## Limits and edge cases

- The WindowServer routes and event fields are private macOS interfaces. Missing symbols return `INTERFACE_UNAVAILABLE`. Each macOS update requires fresh live evidence; older macOS, Intel, and Rosetta are not supported by this contract.
- The whole action, including key-window confirmation, has a ten-second deadline. Application callbacks may continue after Recon stops waiting, and their effects persist even when the command reports failure.
- Recon cannot make arbitrary target code pure or undo its effects. A posted event, an AX return, and an application-level result are different facts; successful delivery is explicitly unverified.
- A background Mac Catalyst window receives its first click only over a control, such as a button. A click on any other view is taken as window activation and never reaches the application, and middle clicks never reach a Catalyst application at all. Recon still sends these clicks and reports success, because it never observes whether the application acted.
- Changing a background target's own key window is allowed and reported. Changing the user's frontmost application, window order, Space, pointer, keyboard focus, or input is interference and fails the operation.
- The final identity check and a process-addressed post have a residual race: a process could exit and its PID could be reused between them. Recon bounds and checks the incarnation but does not claim to eliminate that kernel routing race.
- Do not leave a Catalyst chord half-released. Recon's physical modifier ordering and release tracking exist specifically to prevent a lost Command-flagged key-up from causing repeated commands.
- `mise run test:interaction` requires the AppKit fixture, Catalyst fixture, and foreground receiver to be running already, with the receiver frontmost. It never starts, activates, or stops them. Its negative controls deliberately disturb the desktop and require the operator's explicit permission; the hardware-attribution row may ask for one real keystroke.
