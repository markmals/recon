---
id: guide.macos-application-investigation
title: Investigate macOS Applications
describes: [proposal.0001]
---

# Investigate macOS Applications

## What it does

Recon can list application bundles known to macOS, inspect one bundle and its running process IDs, and launch a selected bundle. `list` and `inspect` read metadata without opening an application, changing the foreground app, moving the pointer, or sending input. `launch` is an explicit action that can bring the selected app to the foreground. These commands do not inspect windows, controls, screenshots, or executable code.

## How to use it

On a Mac with the `recon` executable installed, run `recon app list` to obtain a JSON object with `discovery: "indexed-and-running"` and an `applications` array. Each entry contains a displayed `name`, a `bundleId` that may be `null`, an absolute bundle `path`, and `runningPids`. The list combines indexed application bundles with running apps that expose a bundle location. It is not an exhaustive inventory of every app on every volume. A running app without an identifiable bundle location cannot appear in this list.

Run `recon app inspect <target>` for details of one app. A target can be an exact displayed name, an exact bundle identifier (matched without regard to case), or an absolute `.app` path. An absolute path also works for a valid app that the list did not discover. If a name or identifier matches multiple locations, Recon returns an ambiguity error listing their paths; use the path of the copy you intend to inspect. If a string matches both an app name and another app's identifier, it is ambiguous rather than favoring either one.

Inspection returns an `application` object with `name`, `bundleId`, `path`, `version`, `build`, `executablePath`, and `runningPids`. The bundle identifier, version, build, or executable path may be `null` when unavailable. The process IDs describe instances observed at the time of inspection; an empty array is not a promise that the app will remain stopped. The app's displayed name falls back to its bundle filename when macOS supplies no display name. Neither inspecting nor listing changes the foreground app.

Run `recon app launch <target>` only when you intend to open or activate the selected app. It uses the same targeting rules, does not request a second instance, and does not silently substitute another installed copy. On success the JSON result contains the selected bundle `path` and the host-reported `pid`. Success means macOS opened or activated the app; it does not mean a window or control is ready.

## Worked example

On a Mac with the built-in Calculator app, first inspect it without opening it:

```sh
recon app inspect /System/Applications/Calculator.app
```

The JSON result identifies the Calculator bundle and reports any running process IDs seen at that moment. To open it deliberately, run:

```sh
recon app launch /System/Applications/Calculator.app
```

This second command may foreground Calculator. If you only need to identify the app, stop after `inspect`.

## Limits and edge cases

- Discovery relies on the host's indexed and accessible locations plus running apps with bundle locations. If indexing is unavailable, `list` reports an error instead of returning a misleading partial inventory. Explicit app paths remain usable.
- A missing, unreadable, or non-application path, an unmatched or ambiguous name or identifier, or a launch refusal produces a diagnostic on standard error, a nonzero exit status, and no success JSON.
- If a bundle has no executable file, `launch` reports `application executable is missing` before asking macOS to open it.
- Process IDs can disappear or be reused after the command returns. Use the app's absolute bundle path when referring to a particular installed copy.
- On a non-macOS host these commands explicitly report that this implementation is unavailable; they do not return an empty success result.
- These commands need no Accessibility or Screen Recording grant for their bundle and process metadata. Launch can still encounter ordinary macOS security prompts or errors; Recon does not bypass them.
