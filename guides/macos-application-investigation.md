---
id: guide.macos-application-investigation
title: Investigate macOS Applications
describes: [proposal.0001, proposal.0003, proposal.0004]
---

# Investigate macOS Applications

## What it does

Recon can list application bundles known to macOS, inspect one bundle and its running process IDs, and launch a selected bundle. `list` and `inspect` read metadata without opening an application, changing the foreground app, moving the pointer, or sending input. `launch` is an explicit action that can bring the selected app to the foreground. These commands do not inspect windows, controls, screenshots, or executable code.

## How to use it

On a Mac with the `recon` executable installed, run `recon app list` to obtain a JSON object with `discovery: "indexed-and-running"` and an `applications` array. Each entry contains a displayed `name`, a `bundleId` that may be `null`, an absolute bundle `path`, and `runningPids`. The list combines indexed application bundles with running apps that expose a bundle location. It is not an exhaustive inventory of every app on every volume. A running app without an identifiable bundle location cannot appear in this list.

Run `recon app list --running` to ask the narrower question: which of those applications is running right now. The result holds exactly the entries whose `runningPids` is not empty, with the same fields, the same order (name, then path), and the same ascending process IDs as the full list, and its `discovery` reads `"running"` rather than `"indexed-and-running"`, so a result you keep says which question it answers. When no application is running, `applications` is an empty array and the command still succeeds. `--running` takes no value and may appear once; `--running=yes`, a repeated `--running`, a value after it, or a target beside it is a usage error, and `inspect` and `launch` do not accept it.

Run `recon app inspect <target>` for details of one app. A target can be an exact displayed name, an exact bundle identifier (matched without regard to case), or an absolute `.app` path. An absolute path also works for a valid app that the list did not discover. If a name or identifier matches multiple locations, Recon fails with the code `AMBIGUOUS_TARGET`, whose `details.candidates` lists every matching bundle by `name`, `bundleId`, and `path`; pass the path of the copy you intend to inspect. If a string matches both an app name and another app's identifier, it is ambiguous rather than favoring either one.

Inspection returns an `application` object with `name`, `bundleId`, `path`, `version`, `build`, `executablePath`, and `runningPids`. The bundle identifier, version, build, or executable path may be `null` when unavailable. The process IDs describe instances observed at the time of inspection; an empty array is not a promise that the app will remain stopped. The app's displayed name falls back to its bundle filename when macOS supplies no display name. Neither inspecting nor listing changes the foreground app.

Run `recon app launch <target>` only when you intend to open or activate the selected app. It uses the same targeting rules, does not request a second instance, and does not silently substitute another installed copy. On success the JSON result contains the selected bundle `path` and the host-reported `pid`. Success means macOS opened or activated the app; it does not mean a window or control is ready.

Every command prints one JSON object to standard output and exits 0 on success, or one JSON error object on a single line to standard error and exits nonzero on failure. The error object is `{"error":{"code":…,"message":…,"retryable":…,"command":[…],"details":{…}}}`. Branch on `code`, which comes from a closed set; read `details` for the values you need to recover, such as the candidate bundles of an ambiguity or the reason a path is not an application. `retryable` is `true` only when repeating the identical command could succeed on its own.

To learn the commands without reading this repository, run `recon --help` for the command list, `recon schema app inspect` for one command's arguments, output schema, and error codes, and `recon context` for this installation's version, permissions, and whether application discovery works here. The [CLI contract guide](cli-contract.md) covers orientation, `recon schema`, `recon context`, and how to read an error object.

## Worked example

To see which applications are running before choosing one, run:

```sh
recon app list --running
```

Every entry in `applications` is an application with at least one process ID observed at that moment, and `discovery` reads `"running"`. Nothing is opened, activated, or quit.

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

- Discovery relies on the host's indexed and accessible locations plus running apps with bundle locations. If indexing is unavailable, `list` fails with `DISCOVERY_UNAVAILABLE` instead of returning a misleading partial inventory; that code is retryable, because Spotlight may answer a later attempt. Explicit app paths remain usable.
- `--running` narrows the list, not the discovery behind it. Recon still consults both the index and the running applications, so an application running from a bundle the index does not list is still reported, and an unavailable index is still `DISCOVERY_UNAVAILABLE` rather than a list of running applications alone.
- Every failure prints one JSON error object to standard error, exits nonzero, and prints no success JSON. A path that cannot be read is `NOT_FOUND` with `details.kind: "path"`. A path that exists but is not a readable application bundle is `NOT_AN_APPLICATION`, with `details.reason` of `not-a-bundle-directory`, `unreadable-info-plist`, or `no-name`. A selector that matches nothing is `NOT_FOUND` with `details.kind: "application"` and the selector you passed. A selector that matches several bundles is `AMBIGUOUS_TARGET`. A refused launch is `LAUNCH_FAILED` with `details.reason`.
- Recon treats a directory as an application when its extension is `.app`, in any case, and it contains a readable `Contents/Info.plist` dictionary. As with macOS, the bundle's declared package type does not matter, so system apps such as Finder and Passwords resolve like any other app.
- If a bundle has no executable file, `launch` fails with `LAUNCH_FAILED` and `details.reason: "executable-missing"` before asking macOS to open it; the message still reads `application executable is missing`.
- Process IDs can disappear or be reused after the command returns. Use the app's absolute bundle path when referring to a particular installed copy.
- On a non-macOS host these commands fail with `UNSUPPORTED_HOST`, whose `details` name the host you are on and the hosts the command supports; they do not return an empty success result.
- These commands need no Accessibility or Screen Recording grant for their bundle and process metadata. Launch can still encounter ordinary macOS security prompts or errors; Recon does not bypass them.
