---
id: proposal.0002
title: macOS UI Observation
authors: [Mark Malstrom]
status: awaiting-implementation
pull-request: https://github.com/markmals/recon/pull/26
issues: ["#15"]
supersedes: []
---

# macOS UI Observation

## Summary

Add `recon ui windows`, `recon ui tree`, and `recon ui screenshot` so an agent can see a running macOS app's windows, accessibility tree, and window pixels without activating it, moving the pointer, or sending input. Recon holds its own signed privacy identity, independent of the launching harness.

## Authorization

- Authority: Mark consented on 2026-10-15 to a proposal for issue #15, after reviewing the recommendation that UI observation be the next proposal. No build mandate applies.
- Fit: This proposal is limited to read-only observation of running macOS application windows, the privacy identity those observations require, and the fixture that verifies them.
- Decisions recorded: On 2026-10-15, Mark decided that (1) winctl's private techniques come first, with public-API support added only if they cannot be made to work on this Mac with SIP enabled; grounding showed they work, so public-API capture is out of scope; (2) Recon claims its own stable privacy identity, and Mark will help establish consistent signing; (3) a window selector is layered on top of proposal 0001's bundle target; (4) this proposal builds Recon's first purpose-built native macOS fixture. Later on 2026-10-15, Mark chose `me.malstrom.recon` as Recon's identifier, following his `me.malstrom.*` convention for `malstrom.me`; chose local signing for now, deferring signing in CI until Recon has a Homebrew formula; and approved this proposal for implementation.

## Motivation

Proposal 0001 lets an agent identify a macOS application bundle, its running process IDs, and launch it, but not see it. Feature discovery (#17), experiments that capture screenshots and accessibility state (#18), and control interaction (#24) all need to know which windows a running app has, which menus and controls it exposes, and what a window looks like.

The current workarounds disturb the person using the Mac or produce untrustworthy evidence. `screencapture` and System Events scripting either capture the whole screen, which includes other apps, or bring the target forward first. Permissions for both are granted to whichever terminal or agent harness launched the command, so a command that works under one harness fails under another. Evidence whose availability depends on the launcher cannot be reproduced reliably.

## Domain grounding

Claims marked _observed_ come from direct experiments on 2026-10-15 on this Mac (macOS 27.0.1, Apple silicon, System Integrity Protection enabled, AMFI enforcing), using winctl at commit `295a027`, built and signed with Mark's Developer ID Application certificate. VS Code stayed the frontmost application throughout every observation experiment.

### Established context

**The window server**

- The macOS window server owns every window. Each window has a `CGWindowID`, an owning process ID, a layer, bounds in global points with the origin at the top left of the main display, an alpha value, an on-screen flag, and an optional name. The public [`CGWindowListCopyWindowInfo`](<https://developer.apple.com/documentation/coregraphics/cgwindowlistcopywindowinfo(_:_:)>) enumerates them and is not deprecated. Since macOS 10.15, names of other processes' windows are withheld without Screen Recording permission ([secondary](https://www.ryanthomson.net/articles/screen-recording-permissions-catalina-mess/)). Observed: before the grant, titles were empty; after it, they appeared.
- A window belongs to exactly one process. Observed: the "Open and Save Panel Service" process owned five windows, so a panel that visually belongs to an app can be owned by a system service.
- Window-server windows are not the same as the windows a person perceives. Observed: Safari, Notes, Finder, MarkEdit, and TextEdit each owned several untitled, layer-0, menu-bar-sized (1352×29) windows that correspond to no accessibility window. Finder owned an invisible layer-3 window with alpha 0, whose capture contained no visible pixels.

**Accessibility**

- The Accessibility API reads another process's UI out of process. [`AXUIElementCreateApplication`](https://developer.apple.com/documentation/applicationservices/1459374-axuielementcreateapplication) creates an application's root element, and [`AXUIElementSetMessagingTimeout`](https://developer.apple.com/documentation/applicationservices/1459345-axuielementsetmessagingtimeout) bounds how long one request waits. Reads require the Accessibility permission ([`AXIsProcessTrustedWithOptions`](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions)). Observed: reading did not change the frontmost application.
- An application element lists its menu bar and its windows on the current Space. Observed: Safari, whose only document window was on another Space, reported zero `AXWindows`, yet its `AXMainWindow` attribute returned that window. A minimized TextEdit window remained in `AXWindows`, but TextEdit then reported no main window (`kAXErrorNoValue`).
- The private `_AXUIElementCreateWithRemoteToken` constructs an element from a process ID and a per-process element number, and the private `_AXUIElementGetWindow` maps an accessibility window to its `CGWindowID`. Observed: scanning element numbers found Safari's off-Space window at number 4236 and MarkEdit's at 44. Scanning 5,000 numbers took 0.34 seconds. Element numbers have no documented upper bound.
- Accessibility elements have no durable identity. `AXIdentifier` is optional and assigned by the app's developer. Menu items can be read without opening menus. Observed: MarkEdit exposed 357 menu items. Safari's 3,158-node application tree, almost entirely menus, took 2.9 seconds to read.
- Chromium and Electron build their accessibility trees lazily ([secondary](https://github.com/electron/electron/pull/10305)). Electron investigation is issue #4.

**Window capture**

- `CGWindowListCreateImage` was deprecated in macOS 14 and is unavailable to code built with the macOS 15 SDK ([secondary](https://nonstrict.eu/blog/2023/a-look-at-screencapturekit-on-macos-sonoma/)). Apple's replacement is ScreenCaptureKit; [`SCScreenshotManager`](https://developer.apple.com/documentation/screencapturekit/scscreenshotmanager) needs macOS 14 or later.
- winctl captures one window's backing store with the private SkyLight function `SLSHWCaptureWindowList`. Observed with Screen Recording granted: capture produced 2× images of windows that were occluded, on another Space, minimized, or in a hidden app, without changing the frontmost application. It failed without the grant. Text changed in an occluded window appeared in the next capture. After the content of a minimized window changed, capture failed repeatedly. Safari's off-Space window captured its toolbar with a blank page area.
- AppKit lets applications stop drawing when their windows are not visible ([`NSWindow.occlusionState`](https://developer.apple.com/documentation/appkit/nswindow/occlusionstate)). A captured image is the window server's current content for a window, not proof of what the app would draw if the window were visible.

**Privacy identity**

- macOS's privacy system (TCC) attributes a permission request to the _responsible process_. By default, that is the GUI application from which a command-line tool was launched, such as a terminal or editor ([secondary](https://www.qt.io/blog/the-curious-case-of-the-responsible-process)). The private libSystem function `responsibility_spawnattrs_setdisclaim` makes a spawned child responsible for itself ([secondary](https://github.com/ghostty-org/ghostty/issues/9263)). Observed: a self-disclaimed winctl received none of its launcher's grants, even though its launcher, `omp`, holds Screen Recording. After winctl itself was granted, it worked. The private `responsibility_get_pid_responsible_for_pid` reported another process as responsible for a test program launched from a shell, and the program's own process ID for its disclaimed child.
- Grants are recorded against a client and its code-signing designated requirement ([Apple PPPC payload](https://support.apple.com/guide/deployment/privacy-preferences-policy-control-payload-dep38df53c2a/web)). Observed for a bare executable with an embedded Info.plist, signed with Developer ID so that its requirement names the identifier and team: the grant survived replacing the executable in place with a rebuild whose code hash differed. A copy at another path had no grant. System Settings listed it by its bundle name, "winctl", in both the Screen & System Audio Recording and Accessibility lists.
- Ad-hoc signatures, which the Rust toolchain applies by default on Apple silicon, use the code hash as the designated requirement, so a rebuild loses the grant ([secondary](https://github.com/CapSoftware/Cap/issues/1722)). Observed: one attempt to wrap the probe in an app bundle produced no permission prompt, and the bundle was not listed.
- Since macOS 15, macOS has periodically asked users to reconfirm screen-recording permission ([secondary](https://tidbits.com/2024/08/19/apple-reduces-excessive-sequoia-permission-requests-shifts-to-monthly/)). In macOS 26 and later, the pane is named "Screen & System Audio Recording" (observed).

### Relevant constraints and principles

- [VISION.md](../VISION.md) requires UI adapters to report unavailable capabilities and permission failures explicitly. It also says the suitability of a winctl integration requires separate evaluation. This proposal reimplements selected winctl techniques rather than depending on winctl.
- Proposal 0001's targeting contract applies: a target is an application bundle at a location, an ambiguous selector is an error rather than an arbitrary choice, and reading must not activate the app.
- Private interfaces may change or disappear in any macOS update. Recon must detect a missing interface and report it, rather than crash or return misleading data.
- Inference: a binary freshly built in CI cannot hold privacy grants, so automated CI tests cannot exercise permission-gated behavior.
- Signing certificates are credentials. Placing one in CI and publishing a release each require Mark's explicit action.

### Quality bar

- Observation never activates the target, changes key window or focus, moves the pointer, opens a menu, sends input, performs an accessibility action, or sets an accessibility attribute.
- Every result names the observed process and window, and capture results name their mechanism. An incomplete result says so through truncation markers, timeouts, missing-permission errors, or unavailable-mechanism errors, rather than appearing complete.
- Permissions belong to Recon. They behave the same under any launching terminal or harness and survive in-place upgrades.
- An ambiguous instance or window is an error that lists the candidates. Recon never picks the largest, first, or frontmost one.

### Remaining uncertainty

- The private SkyLight and Accessibility interfaces have been verified only on macOS 27.0.1 on Apple silicon.
- It is unknown whether macOS's periodic screen-recording reminder applies to SkyLight capture.
- An accessibility window whose element number exceeds Recon's scan limit cannot be reached through `--window`.
- Reading a closed menu can make an app run its own menu-update code. No visible change was observed.

## Existing baseline

[winctl](https://github.com/NSExceptional/winctl) demonstrates the techniques on a SIP-disabled research Mac. Grounding verified the following on a stock Mac, and this proposal preserves them:

- resolving private symbols at runtime with `dlsym`, so nothing private is linked;
- SkyLight hardware window capture;
- re-executing the tool once with the disclaim spawn attribute;
- embedding an Info.plist in the executable and signing with a stable designated requirement;
- read-only accessibility traversal.

These parts are incidental or compromised, and are not carried forward:

- **Target selection.** Substring app-name matching and choosing the largest window are arbitrary selection.
- **Unbounded traversal.** Walking the whole app without a messaging timeout or cycle guard lets an unresponsive or cyclic app hang it.
- **Silent identity change.** If disclaiming fails, winctl silently falls back to the launcher's identity.
- **Obsolete capture fallback.** Its fallback is the obsoleted `CGWindowListCreateImage`.
- **Direct TCC database access.** It reads the TCC database directly, which needs Full Disk Access and returned "unreadable" here. Its `grant` command writes that database, which works only with SIP disabled.
- **Third-party signing identity.** Its signing identity is hard-coded to its author's certificate.

Recon's existing `app` commands supply the rest: bundle resolution, explicit errors on standard error, one JSON object on success, and an explicit error on hosts other than macOS.

## Proposed solution

Add a `ui` command group that observes one running instance of a resolved bundle. `windows` reports the window server's facts about that instance's windows. `tree` reads the accessibility tree of the application or one of its windows. `screenshot` captures one window's current pixels to a PNG file. Each command checks the permission it needs and reports a missing one explicitly. It then reads without changing what the user sees or which app has focus.

`ui` commands run as their own responsible process, so their grants belong to the `recon` executable rather than the terminal or harness that launched it. The executable carries an embedded Info.plist and a stable Developer ID signature, so grants survive rebuilds installed at the same path.

```text
recon ui windows "Logic Pro" → the instance's windows with IDs, titles, layers, bounds, and visibility
recon ui tree "Logic Pro" → menu bar and current-Space windows as accessibility nodes
recon ui tree "Logic Pro" --window 53510 → the accessibility tree of that window, even on another Space
recon ui screenshot "Logic Pro" --out bounce.png → PNG of the app's main window, Logic Pro still in the background
```

## Detailed design

### Concepts

- **Application**: a bundle at a location, resolved exactly as in proposal 0001.
- **Instance**: a running process of that bundle, identified by its process ID. The ID is valid only while the process runs and can be reused afterwards.
- **Window**: a window-server window owned by the instance, identified by its window ID. The ID is valid only while the window exists in the current login session.
- **Accessibility element**: a node the application exposes through Accessibility. It has no durable identity, so a node is identified only by its position within one tree result. Some windows have an accessibility window; others, such as menu-bar windows, do not.
- **Main window**: the window the application itself reports as its main window through Accessibility.

### Selecting an instance and a window

- Flags may come before or after the target. A missing target, an extra positional argument, a repeated flag, a flag the command does not take, or a missing or malformed flag value is a usage error.
- Every `ui` command takes a target that is resolved under proposal 0001's rules, including its ambiguity and not-found errors. Recon then selects one running instance of that bundle:
    - If the bundle has exactly one running process, Recon selects it.
    - If it has none, Recon fails with `<path> is not running; recon ui does not launch applications`.
    - If it has several, Recon fails with an ambiguity error that lists their process IDs, unless `--pid <pid>` names one of them.
    - A `--pid` that is not a running process of the resolved bundle is an error.
- The windows of an instance are exactly those the window server reports as owned by its process ID. Windows owned by other processes, including system services that present panels for the app, are not included.
- `--window <id>` selects a window by window ID. It must exist and be owned by the selected instance; otherwise the command fails and names the window ID and process ID.

### `recon ui windows <target> [--pid <pid>]`

- Requires Screen Recording. It does not read Accessibility.
- Reports every window owned by the instance in the window server's front-to-back order. That includes windows on screen and off screen, at every layer, size, and alpha.
- Output: `{"application":{"path":string,"pid":integer},"windows":[{"id":integer,"title":string|null,"layer":integer,"bounds":{"x":number,"y":number,"width":number,"height":number},"onScreen":boolean,"alpha":number},...]}`.
    - Bounds are in global points from the top left of the main display.
    - `title` is `null` when the window server reports no title or an empty one.
    - `onScreen: false` does not distinguish between a window on another Space, a minimized window, a window in a hidden app, and a window that has been ordered out.

### `recon ui tree <target> [--pid <pid>] [--window <id>]`

- Requires Accessibility.
- Without `--window`, the root is the application element and its children are those the application reports, typically its menu bar and its windows on the current Space. Windows on other Spaces are usually absent, and Recon does not add them.
- With `--window`, the root is that window's accessibility element. Recon looks for it in this order:
    1. the application's `AXWindows`;
    2. its `AXMainWindow`;
    3. the private remote-token construction for element numbers 0 through 99,999, matched by `_AXUIElementGetWindow`.

    If none matches, the command fails with `window <id> has no accessibility element reachable by Recon`. If the application does not answer one of these requests within the messaging timeout, Recon stops looking and the command fails with `process <pid> did not answer an accessibility request within 2 seconds`.

- Each element becomes a node with this shape: `{"role":string|null,"subrole":string|null,"title":string|null,"description":string|null,"value":string|number|boolean|null,"identifier":string|null,"enabled":boolean|null,"focused":boolean|null,"frame":{"x":number,"y":number,"width":number,"height":number}|null,"actions":[string,...],"children":[node,...]}`.
    - Fields come from the attributes `AXRole`, `AXSubrole`, `AXTitle`, `AXDescription`, `AXValue`, `AXIdentifier`, `AXEnabled`, `AXFocused`, `AXPosition` with `AXSize`, and `AXChildren`, and from the element's action names.
    - A missing or unsupported attribute is `null`, or `[]` for `actions` and `children`.
    - A value that is not a string, number, or boolean is `null`.
    - A string value longer than 4,096 Unicode scalar values is cut to its first 4,096, and the node carries `"valueTruncated":true`.
    - The frame is in global points and is `null` unless both position and size are available.
- Traversal:
    - Recon follows only `AXChildren`, depth first, in the order the application reports.
    - The root has depth 0. Recon does not read the children of nodes at depth 64.
    - A result holds at most 25,000 nodes, counting repeated and failed nodes. Once it holds 25,000, Recon reads no further elements.
    - A node with any child that Recon did not read carries `"childrenOmitted":true`. A child that is not an accessibility element is not read.
    - An element already emitted in the same result appears again as `{"role":string|null,"repeated":true}`, with its role and without children.
    - An element whose request fails becomes a failed node, without children:
        - `{"error":"element-unavailable"}` when the element no longer exists (`kAXErrorInvalidUIElement`);
        - `{"error":"timeout"}` when the request does not complete in time (`kAXErrorCannotComplete`);
        - `{"error":"read-failed","axError":integer}` for any other failure, with the Accessibility error code.
- Recon sets a messaging timeout of two seconds per request. After a timeout, Recon reads no further elements and returns the partial tree.
- Output: `{"application":{"path":string,"pid":integer},"windowId":integer|null,"truncated":boolean,"root":node}`. `truncated` is `true` whenever any node carries `childrenOmitted` or a timeout ended reading.
- Recon never sets an attribute, performs an action, or opens a menu.

### `recon ui screenshot <target> [--pid <pid>] [--window <id>] --out <file>`

- Requires Screen Recording.
- Without `--window`, the window is the application's main window as reported by Accessibility and mapped to its window ID, so Accessibility is also required. If the application reports no main window, the command fails and suggests choosing one with `recon ui windows` and `--window`. If it does not answer within the messaging timeout, the command fails with `process <pid> did not answer an accessibility request within 2 seconds`.
- `--out` is required. Its parent directory must exist, and the file must not exist; Recon never overwrites. Recon writes a PNG file regardless of the file name's extension, and the output reports its absolute path.
- Capture uses the private SkyLight hardware window capture of exactly one window, at that window's backing pixel resolution. There is no fallback mechanism.
    - If the private symbols are unavailable, the command fails with `window capture is unavailable on this macOS version (SkyLight capture API not found)`.
    - If macOS returns no image, for example for a minimized window whose content has changed, the command fails with `macOS returned no image for window <id>`.
    - No file is written when the command fails before writing. If writing the PNG itself fails, Recon removes the partial file; if macOS refuses that removal, the error names the file left behind.
- Output: `{"application":{"path":string,"pid":integer},"window":{"id":integer,"title":string|null,"bounds":{...},"onScreen":boolean},"image":{"path":string,"width":integer,"height":integer,"uniform":boolean},"method":"skylight-hardware-capture"}`.
    - `width` and `height` are in pixels.
    - `uniform` is `true` when every pixel has the same RGBA value, as in an entirely transparent or entirely black image. Recon records this rather than treating it as an error.
    - `window` describes the window at the time of capture.
- The image is the window server's current content for the window. An application may not redraw a window that is not visible, so the image may lag the application's state; `onScreen` is reported to help interpret it.

### Privacy identity and permissions

- The `recon` executable embeds an Info.plist in its `__TEXT,__info_plist` section with `CFBundleIdentifier` set to `me.malstrom.recon` and `CFBundleName` set to `recon`, so System Settings lists it as "recon". Its code-signing identifier is the same identifier.
- Every `ui` command first ensures that `recon` is responsible for itself, as reported by the private `responsibility_get_pid_responsible_for_pid`. If it is not, `recon` re-executes itself once with the private disclaim spawn attribute.
    - The child receives the same arguments, environment, and standard streams, and `recon` exits with the child's status. `recon` forwards SIGHUP, SIGINT, SIGQUIT, and SIGTERM to the child, and if the child is terminated by a signal, `recon` terminates itself with the same signal, so a caller that stops `recon` also stops the observation. A signal that the launcher set to be ignored, as `nohup` does for SIGHUP, stays ignored by both processes and is not forwarded.
    - If the disclaim interfaces are unavailable, spawning fails, or the child is still not responsible for itself, the command fails rather than run under the launcher's permissions.
    - `app` commands need no privacy permission and do not re-execute.
- Before observing, a `ui` command checks each permission it needs, without prompting: Screen Recording with `CGPreflightScreenCaptureAccess` and Accessibility with `AXIsProcessTrusted`. If one is missing, Recon asks macOS to request it with `CGRequestScreenCaptureAccess` or `AXIsProcessTrustedWithOptions`. macOS may show a prompt, but will not if the user previously denied it. Recon then exits with a nonzero status and an error that names:
    - the permission;
    - the absolute path of the running `recon` executable;
    - the System Settings location: Privacy & Security → Screen & System Audio Recording, or Privacy & Security → Accessibility.

    If the executable is ad-hoc signed, the error adds that macOS will forget the grant when this executable is rebuilt, and that signing it with a Developer ID Application identity keeps the grant across rebuilds.

- Recon is signed locally, not in CI. Builds produced by `cargo build` or CI are ad-hoc signed. Signing an executable with a Developer ID Application identity and the identifier `me.malstrom.recon` gives it a designated requirement that names that identifier and the identity's team, so a grant survives a rebuild signed the same way and installed at the same path. Moving the executable requires granting again.
- The guide documents the `codesign` command that re-signs a built `recon` executable with a given identity and the identifier `me.malstrom.recon`.
- A repository task builds `recon` and signs it at `target/signed/recon` with the signing identity named by the `RECON_SIGNING_IDENTITY` environment variable, for local development and smoke testing. Without that variable, the task fails rather than produce an ad-hoc binary.

### Fixture

- A purpose-built native AppKit fixture lives in the repository as a single Swift source file and Info.plist. A repository task builds it into `target/fixtures/ReconFixture.app` with the Xcode command-line tools.
- It has two titled windows, "Recon Fixture — Main" (its main window) and "Recon Fixture — Inspector". The main window contains a button labelled "Record", a text field with the value "fixture value", and a checkbox labelled "Enabled". A "Fixture" menu contains "First Item" and "Second Item".
- Launching it with `open -g` leaves another application frontmost.

### Other hosts

- On a host other than macOS, `ui` commands report that this implementation does not support that host, as `app` commands do.

## Compatibility

`app` commands keep their syntax and output. The `ui` group is new. The executable gains an embedded Info.plist, which does not change `app` behavior. Recon stores no data to migrate.

## Implications on adoption

- Users sign `recon` locally with their own Developer ID Application identity, using the documented command, and then grant Screen Recording and Accessibility to it once in System Settings. Grants belong to the executable at its installed path. An in-place upgrade signed with the same identifier and team keeps them; moving the executable requires granting again.
- An ad-hoc–signed build from `cargo build` or CI works after granting, but loses its grants whenever it is rebuilt or replaced.
- Since macOS 15, macOS has periodically asked users to reconfirm screen-recording permission, which may interrupt unattended use.
- Recon depends on private macOS interfaces verified only on macOS 27.0.1 on Apple silicon. A macOS update can remove them; Recon then reports the capability as unavailable rather than misbehaving.
- Adoption is reversible: revoke the grants in System Settings and remove the executable.

## Scope

- `recon ui windows`, `recon ui tree`, and `recon ui screenshot`, including instance and window selection, permission checks, and the errors described above.
- The self-disclaiming re-execution, the embedded Info.plist, the documented local signing command, and the signing task.
- The fixture application and its build task. The existing macOS CI artifact gains the fixture alongside the ad-hoc–signed `recon`.
- Tests:
    - CI tests, independent of macOS permissions, for instance and window selection, traversal limits, repeated and vanished elements, value truncation, `uniform` detection, output shapes, and error messages.
    - macOS integration tests that run only when explicitly enabled on a Mac where the signed `recon` holds both grants. They launch the fixture in the background and check its known titles, labels, values, and menu items; non-uniform screenshots of the expected size; an unchanged frontmost application; and an unchanged pointer location.
- A guide, `guides/macos-ui-observation.md`, covering permissions and signing, install location, instance and window selection, outputs, and limits: Spaces, minimized windows, windows owned by helper processes, and private-interface risk.

### Out of scope

- Clicking, typing, accessibility actions, and attribute writes. These are interaction, issue #24.
- ScreenCaptureKit and `CGWindowListCreateImage` capture. The private path works on the target host, per Mark's decision recorded above.
- Whole-display or region screenshots, video, and change notifications. None is needed to observe one window.
- Enabling Chromium or Electron accessibility (#4), web pages (#5), and Windows (#16).
- Persisting observations (#23).
- Signing in CI and storing a signing certificate in CI. Mark deferred both until Recon has a Homebrew formula.
- Release, notarization, package-manager distribution, and app-bundle packaging. None is authorized, and the bare signed executable already holds stable grants.
- Including off-Space windows in the application-level tree, and identifying Spaces or minimized state in `ui windows`. Off-Space windows remain reachable with `--window`.

## Preview

- Artifact: the macOS CI artifact for the draft pull request, containing an ad-hoc–signed `recon` executable for Apple silicon and the built fixture. Recon is not signed in CI.

    The reviewer re-signs `recon` locally with their Developer ID Application identity using the command in the guide, installs it at a stable path, and grants both permissions once. They then launch the fixture with `open -g` and run all three commands while another application stays frontmost. Finally, they run `ui windows`, `ui tree --window`, and `ui screenshot` against a real application on another Space. The artifact keeps normal pull-request artifact expiration and is not published as a release.

- Reason: privacy-identity behavior and private-interface behavior exist only on a real Mac running a signed executable. Re-signing the CI build locally exercises the same signing path a user follows; rendered documentation cannot exercise either.

## Policies and decisions checked

None. `policies/` and `decisions/` contain only README files. Recon's privacy identifier, `me.malstrom.recon`, is knowledge that outlives this change. At completion, it may warrant a decision record, with Mark's approval.

## Future directions

- If SkyLight capture disappears from a future macOS, a ScreenCaptureKit capture path could replace or supplement it.
- Interaction (#24) could reuse the privacy identity and the instance and window selection defined here.
- An evidence store (#23) could record screenshots and trees with their window, instance, application version, and capture conditions.
- Off-Space windows could be added to application-level trees by element-number scanning, and Space membership could be reported in `ui windows`.
- An app-bundle privacy identity could make grants independent of install path, alongside notarized distribution.
- Signing in CI could produce ready-to-grant builds once a Homebrew formula distributes them.

## Alternatives considered

- **Public APIs only (ScreenCaptureKit and public Accessibility).** Rejected for this proposal. Mark chose private techniques first, and they work here with SIP enabled. Public Accessibility also cannot reach most windows on other Spaces.
- **Keeping winctl's `CGWindowListCreateImage` fallback.** Rejected. The function is obsolete, and a second capture mechanism would need its own verification without a demonstrated need.
- **Running winctl as a dependency.** Rejected. It is a single-author research tool with a hard-coded signing identity and no stable interface, and Recon needs its own identity regardless.
- **Using the launcher's permissions.** Rejected. Grants would differ between terminals and harnesses, which is the problem this proposal removes.
- **Signing pull-request previews in CI with a certificate stored in GitHub Actions secrets.** Deferred by Mark until Recon has a Homebrew formula. Local signing exercises the same identity without placing a certificate in CI.
- **An app-bundle wrapper for the privacy identity.** Deferred. One attempt produced no prompt, while the bare signed executable is verified to work.
- **Choosing the largest or frontmost window by default.** Rejected as arbitrary. The application's own main window is a choice the application makes, and is reachable even on another Space.
- **Scanning element numbers for every window in `ui windows` or application trees.** Rejected. Scanning to the limit costs several seconds, and menu-bar windows never match, so every call would pay the worst case.

## Open questions

None.

## Acknowledgments

Tanner Bennett's [winctl](https://github.com/NSExceptional/winctl) research identified the SkyLight capture, self-disclaim, and stable-signing techniques that this proposal verifies and adopts.
