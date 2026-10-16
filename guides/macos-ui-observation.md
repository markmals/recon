---
id: guide.macos-ui-observation
title: Observe macOS Application UI
describes: [proposal.0002]
---

# Observe macOS Application UI

## What it does

Recon can report the windows of a running macOS application, read that application's accessibility tree, and save one window's current pixels to a PNG file. The commands are `recon ui windows`, `recon ui tree`, and `recon ui screenshot`. Each one observes an application that is already running and leaves it exactly where it was: it never activates the app, changes the key window or keyboard focus, moves the pointer, clicks, types, opens a menu, performs an accessibility action, or writes an accessibility attribute. None of them launches anything; `recon ui` against a stopped app is an error, and opening an app remains the job of `recon app launch`. When part of a result cannot be read, Recon says so through an error, a truncation marker, or a failed node rather than returning a result that looks complete.

Observation requires two macOS privacy permissions, and they belong to the `recon` executable rather than to the terminal, editor, or agent harness that ran it. A command therefore behaves the same under every launcher.

## How to use it

### Sign `recon` and grant it permission

`ui` commands work on macOS only. On any other host they report `UI observation is supported only on macOS` and exit with a nonzero status.

Put `recon` at the path where you intend to keep it before granting anything, because macOS records a permission grant against an executable at a location. Then sign it with your own Developer ID Application identity and Recon's identifier:

```sh
codesign --force --sign "Developer ID Application: <Your Name> (<TEAMID>)" --identifier me.malstrom.recon /path/to/recon
```

That signature gives the executable a designated requirement naming the identifier `me.malstrom.recon` and your team. A later build signed the same way and written over the same path keeps the permissions you granted, which is what makes in-place upgrades painless. Moving or copying the executable breaks this: the copy at the new path is a different client to macOS and holds no grant until you grant it again.

Grant the permissions once. Run a `ui` command, let macOS show its prompt, then open System Settings and enable `recon` in Privacy & Security → Screen & System Audio Recording and in Privacy & Security → Accessibility. Both lists show the executable as "recon", from the Info.plist embedded in it.

Which permission a command needs depends on what it reads. `recon ui windows` needs Screen Recording, because macOS withholds other processes' window titles without it, and it reads no accessibility data at all. `recon ui tree` needs Accessibility. `recon ui screenshot` needs Screen Recording to capture pixels, and it needs Accessibility as well whenever you omit `--window`, because the default window is the one the application reports as its main window through Accessibility.

When a needed permission is missing, Recon asks macOS to request it. macOS may show its prompt, and will stay silent if you denied the permission earlier. Recon then exits with a nonzero status and writes an error naming each missing permission, the absolute path of the running executable, and the System Settings pane that grants it:

```text
recon: missing permission for /usr/local/bin/recon:
  Screen Recording: System Settings → Privacy & Security → Screen & System Audio Recording
```

If both permissions are missing, the header is plural and an `Accessibility` line joins it. If the executable is ad-hoc signed, a final line says so, warns that macOS forgets the grant when the executable is rebuilt, and points at a Developer ID Application identity as the fix.

Ad-hoc signing is what `cargo build` and CI produce on Apple silicon, and the designated requirement of an ad-hoc signature is the code hash itself. Such a build works after you grant it, then loses the grant the moment it is rebuilt or replaced, because the new binary hashes differently. Developer ID signing with the identifier above is the way out.

Since macOS 15, macOS periodically asks you to reconfirm screen-recording permission. Expect that to interrupt long unattended runs.

Every `ui` command first makes sure `recon` is responsible for its own privacy requests. If it is not, `recon` re-executes itself once with the same arguments, environment, and standard streams, and exits with the child's status; you see nothing but the result. Stopping `recon` with SIGHUP, SIGINT, SIGQUIT, or SIGTERM stops the re-executed child too, and a child ended by a signal ends `recon` with the same signal. A signal your launcher ignores, as `nohup` ignores SIGHUP, stays ignored by both. If that re-execution is impossible, the command fails instead of quietly running under the launcher's permissions. The `app` commands need no permission and never re-execute.

Working on Recon itself, build and sign a local copy in one step:

```sh
RECON_SIGNING_IDENTITY="Developer ID Application: <Your Name> (<TEAMID>)" vp run recon:sign
```

The task builds the release executable, signs it at `target/signed/recon`, and fails outright when `RECON_SIGNING_IDENTITY` is unset rather than leaving you an ad-hoc binary that appears to work.

Once `target/signed/recon` holds both permissions, `vp run test:ui` with the same variable builds the purpose-built fixture app at `target/fixtures/ReconFixture.app`, re-signs `recon`, and runs the integration tests against the fixture in the background. Those tests check that the frontmost application and the pointer do not change, so leave the keyboard and mouse alone while they run.

### Choose an instance and a window

Every `ui` command takes the same target as `recon app inspect`: an exact displayed name, an exact bundle identifier, or an absolute `.app` path, resolved under the rules in the [macOS application investigation guide](macos-application-investigation.md), including its not-found and ambiguity errors.

Recon then selects one running process of that bundle. A bundle with exactly one running process gives Recon no choice to make. If it has none, the command fails with `<path> is not running; recon ui does not launch applications`. If it has several, Recon refuses to guess and lists them: `<path> has several running instances; choose one with --pid: 410, 902`. Naming a process that is not an instance of the resolved bundle fails too, with `process 411 is not a running instance of <path>`.

The windows of that instance are exactly the ones the window server reports as owned by its process ID. Windows that another process owns are not included, even when they look like part of the app. A save or open panel is the usual case: it belongs to a system service's process, so it appears in neither the window list nor the tree of the app that triggered it.

`--window <id>` selects one window by window ID, which you normally read from `recon ui windows`. The window must exist and be owned by the selected instance; otherwise the command fails with `window <id> is not a window of process <pid>`. Flags may come before or after the target, and passing a flag the command does not take, such as `--window` to `windows`, is a usage error.

### `recon ui windows <target> [--pid <pid>]`

The result is one JSON object. `application` holds the resolved bundle `path` and the observed `pid`. `windows` is an array in the window server's front-to-back order, listing every window the instance owns: on screen and off screen, at any layer, size, and alpha.

Each entry has an `id`, the window ID used by `--window`; a `title`, which is `null` when the window server reports no title or an empty one; a `layer`, the window server's stacking layer for that window; `bounds` with `x`, `y`, `width`, and `height` in global points measured from the top left corner of the main display; `onScreen`; and `alpha`, where 0 is fully transparent and 1 fully opaque.

Read `onScreen: false` carefully. It does not distinguish a window on another Space from a minimized window, a window belonging to a hidden app, or a window that has simply been ordered out.

### `recon ui tree <target> [--pid <pid>] [--window <id>]`

Without `--window`, the root of the tree is the application element, and its children are whatever the application reports, typically its menu bar and its windows on the current Space. Windows on other Spaces are usually missing from that list, and Recon does not go looking for them here.

With `--window`, the root is that window's accessibility element. Recon looks for the element in the application's `AXWindows`, then in its `AXMainWindow`, then by constructing elements from private remote tokens for element numbers 0 through 99,999 and matching their window IDs. A window that none of those reaches fails with `window <id> has no accessibility element reachable by Recon`. An application that does not answer one of those requests within two seconds stops the search, and the command fails with `process <pid> did not answer an accessibility request within 2 seconds`.

The object around the tree is `application` with `path` and `pid`, `windowId` holding the window ID you asked for or `null` when you did not, `truncated`, and `root`.

An ordinary node carries `role`, `subrole`, `title`, `description`, `value`, `identifier`, `enabled`, `focused`, `frame`, `actions`, and `children`. They come from the attributes `AXRole`, `AXSubrole`, `AXTitle`, `AXDescription`, `AXValue`, `AXIdentifier`, `AXEnabled`, `AXFocused`, `AXPosition` combined with `AXSize`, and `AXChildren`, plus the names of the actions the element advertises. An attribute the element does not have or does not support is `null`, and `actions` and `children` are then `[]`. A `value` is reported only when it is a string, a number, or a boolean; anything else is `null`. A string `value` longer than 4,096 Unicode scalar values is cut to its first 4,096 and the node gains `"valueTruncated": true`. The `frame` is in the same global points as window bounds and is `null` unless both position and size are available. Recon lists the actions; it never performs one.

Two other node shapes appear in a tree:

- A node Recon has already emitted in this result appears again as `{"role": <role>, "repeated": true}`, with no children. Accessibility elements have no durable identity, so this marks a structure that points back at something you have already seen rather than giving you a second copy of it.
- A node whose request failed carries an `error` and no children: `{"error": "element-unavailable"}` when the element no longer exists, `{"error": "timeout"}` when the request did not complete in time, and `{"error": "read-failed", "axError": <code>}` for any other failure, with the Accessibility error code.

Reading is bounded in four ways. Recon follows only `AXChildren`, depth first, in the order the application reports them. The root is depth 0, and Recon does not read the children of nodes at depth 64. A result holds at most 25,000 nodes, counting repeated and failed ones, and reading stops there. Each request gets a two-second messaging timeout, and after one request times out Recon reads no further elements and returns the partial tree.

Any node whose children were not all read carries `"childrenOmitted": true`, including a node that listed a child that is not an accessibility element. The top-level `truncated` is `true` whenever some node carries `childrenOmitted` or a timeout ended the read, so one field tells you whether the tree is the whole story.

### `recon ui screenshot <target> [--pid <pid>] [--window <id>] --out <file>`

`--out` is required, and Recon never overwrites. A missing parent directory fails with `cannot write <path>: <parent> is not an existing directory`, and a path that already exists fails with `cannot write <path>: the file already exists; recon never overwrites`. The file is written as PNG whatever extension you give it.

Without `--window`, Recon captures the window the application reports as its main window through Accessibility. An application that reports no main window, which happens for instance when its only window is minimized, makes the command fail with a suggestion to list the windows with `recon ui windows` and pick one with `--window`. An application that does not answer within two seconds makes it fail with `process <pid> did not answer an accessibility request within 2 seconds`.

Capture uses a private SkyLight hardware capture of exactly one window, at that window's backing pixel resolution, with no fallback mechanism. Two failures are reported plainly, and neither writes a file. If the private symbols are missing, the command fails with `window capture is unavailable on this macOS version (SkyLight capture API not found)`. If macOS returns no image, the command fails with `macOS returned no image for window <id>`. If writing the PNG itself fails, Recon removes the partial file, and if macOS refuses even that, the error names the file left behind.

On success the object holds `application` with `path` and `pid`; `window` with the `id`, `title`, `bounds`, and `onScreen` of the window as it was at the moment of capture; `image` with the absolute `path` written, the `width` and `height` in pixels, and `uniform`; and `method`, always `"skylight-hardware-capture"`. Pixel dimensions are the window's backing resolution, so on a Retina display they are larger than the point dimensions in `bounds`. `uniform` is `true` when every pixel has the same RGBA value, as in an entirely transparent or entirely black capture; Recon records that fact instead of calling it an error.

What you get is the window server's current content for that window. An application is allowed to stop drawing a window nobody can see, so a capture of an occluded, off-Space, minimized, or hidden window may lag what the application would draw if the window were visible. `onScreen` is in the output to help you judge how much to trust the image.

## Worked example

The built-in Calculator is a safe subject. Start it in the background, which leaves whatever you are using frontmost:

```sh
open -g /System/Applications/Calculator.app
```

List its windows:

```sh
recon ui windows /System/Applications/Calculator.app
```

The result names the bundle path and the process ID, then the windows the window server attributes to that process. More than one entry is normal, since apps own windows that nobody would point at, so read the titles, bounds, and `onScreen` flags instead of assuming the first entry is the calculator itself. Take the `id` of the window you want and use it below as `<id>`.

Read that window's accessibility tree:

```sh
recon ui tree /System/Applications/Calculator.app --window <id>
```

`windowId` echoes `<id>`, and `root` is the window element, with the buttons and display of the calculator below it as nodes carrying roles, titles, frames, and the action names each element advertises. Recon reads those names; it presses nothing. Drop `--window` to get the application element instead, with Calculator's menu bar and its windows on the current Space.

Capture the same window:

```sh
recon ui screenshot /System/Applications/Calculator.app --window <id> --out ~/Desktop/calculator.png
```

The result reports the absolute path of the PNG, its pixel width and height, and whether the image is uniform. Omit `--window` to capture Calculator's main window instead, which also needs Accessibility. Calculator stays in the background through all three commands, the frontmost application does not change, and the pointer does not move.

## Limits and edge cases

- A window ID is valid only while that window exists in the current login session, and a process ID only while the process runs, after which macOS may reuse it. Treat both as short-lived and resolve them again rather than storing them.
- Accessibility elements have no durable identity. A node is identified only by its position inside one tree result, and `identifier` is filled in only when the application's developer assigned one.
- A window on another Space is usually absent from an application-level tree. It stays reachable through `--window`, and its screenshot works, but an accessibility window whose private element number exceeds 99,999 cannot be reached at all.
- An app often owns windows that no person perceives as windows, such as untitled menu-bar-sized ones, and may own invisible windows with alpha 0. `recon ui windows` lists them because the window server reports them.
- Capturing a minimized window can fail repeatedly once its content has changed, with `macOS returned no image for window <id>` and no file written.
- SkyLight capture and the private accessibility interfaces behind `--window` have been verified only on macOS 27.0.1 on Apple silicon. A macOS update can remove them; Recon then reports the capability as unavailable rather than crashing or returning misleading data.
- Reading a closed menu can make an application run its own menu-update code. Recon does not open menus, and no visible change was observed from reading them.
- Every failure, including a missing permission, an ambiguous instance, an unowned window, and an existing `--out` path, produces a diagnostic on standard error, a nonzero exit status, and no success JSON.
- Grants are tied to the executable at its path. Rebuilding an ad-hoc binary, moving a signed one, or running a second copy from another directory all mean granting again in System Settings.
- Adoption is reversible: revoke the two grants in System Settings and delete the executable.
