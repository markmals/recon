---
id: guide.cli-contract
title: Discover and Diagnose Recon
describes: [proposal.0003]
---

# Discover and Diagnose Recon

## What it does

Recon describes its commands through help and a machine-readable schema. `recon context` reports the installed executable's identity, permissions, and host capabilities without requesting any permission or touching a target application.

## How to use it

Run `recon`, `recon --help`, `recon -h`, or `recon help` for orientation. Help lists every command and the discovery commands. For a group or command, append `--help` or `-h`, or put its path after `recon help`. Help succeeds on standard output even when other arguments are malformed. Help has no terminal colors and is identical when redirected.

Run `recon --version` for the crate version and, when available at build time, the short Git commit and its author date. A build without eligible Git metadata prints only the version. `-v` and `-V` are usage errors.

The version suffix and `context.recon.commit` report the checkout's `HEAD` baseline when the Git root matches this workspace and the committed `crates/recon/Cargo.toml` names package `recon`. Missing, malformed, or nonmatching metadata is omitted. Dirty dependency/version edits and linked worktrees keep that baseline. A same-name archive overlaid on another checkout can also qualify: this name-based check cannot attest to source origin or byte-for-byte correspondence. No remote or history allowlist is consulted.

The next build refreshes metadata when `HEAD` moves or a source archive becomes a checkout. Changes to a linked or separate checkout's `.git` locator are also observed.

`recon schema` returns the command index in stable order, with effects, supported hosts, and the complete error contract. `recon schema <command>` returns that command's arguments, options, constraints, defaults, effects, permissions, supported hosts, output JSON Schema, and possible error codes. `recon schema schema` and `recon schema context` describe the discovery commands themselves. An unknown schema path is an `INVALID_ARGUMENT` error listing accepted paths.

Output schemas use JSON Schema 2020-12. Each is self-contained; local `$defs` resolve every reference without a network request. Objects Recon controls reject unknown properties. The schemas describe serialized output, including optional truncation markers and the repeated and failed node forms of an accessibility tree.

Options use long names and take one value. Options may precede or follow the target. Repeated or unknown options, missing values, a value beginning with a hyphen, and extra positionals are errors. Numbers contain ASCII decimal digits only, within the unsigned 32-bit range; `--pid` must exceed zero. A group alone requires an operation. Only an empty invocation shows top-level help automatically.

## Read the execution context

`recon context` returns `recon` with version, full build commit or `null`, and the canonical absolute executable path. `host` gives the compiled target OS and architecture; its version is the structured macOS product version or `null` on other hosts. `macos` is `null` outside macOS.

On macOS, the report includes code-signing identifier and team, signed and ad-hoc flags, whether Recon is responsible for its own privacy identity, preflight results for Screen Recording and Accessibility, the six private interfaces it uses, and application-discovery availability. Permission entries include the System Settings pane that grants each permission. An identity failure before spawning is reported with a reason and `granted: null` for both permissions. Those grants are unknown; the launcher's permissions cannot answer the question.

Context re-executes when necessary to report Recon's own grants. It never asks macOS to request a permission, never shows a permission dialog, and succeeds when permissions or private interfaces are missing. Discovery availability measures Recon's own Spotlight query and indexed application count, rather than Spotlight's system-wide configuration. Ad-hoc builds lose grants on rebuild; this report does not change signing or grants.

## Read a failure

Every failure produces exactly one compact JSON object on one newline-terminated line of standard error, with no prose prefix and no success output. `error.code` classifies the failure, `message` explains it, `retryable` says whether repeating the identical command can succeed, `command` gives the resolved path or `null`, and `details` provides recovery data or `null` when there is none. Use codes and details rather than matching English messages.

Exit status is 0 on success, 2 for `INVALID_ARGUMENT` failures detectable from the command line alone, and 1 for runtime failures. Signals retain their normal shell behavior, including when Recon re-executes.

When Recon re-executes, the parent withholds both child output streams until it has successfully waited for the child. It drains both streams concurrently, so large results cannot deadlock against full pipes. After a successful wait, each stream's bytes and the child's exit status are preserved. A genuine wait failure discards all child output and produces only `IDENTITY_UNAVAILABLE` with `details.stage: "wait"`, exit 1, and empty standard output. This applies to `context` too: a post-spawn wait failure never produces a fallback context report.

For an invalid argument, inspect `argument`, `value`, `expected`, and `accepted`. For `AMBIGUOUS_TARGET`, choose one of `candidates` by bundle path; for `AMBIGUOUS_INSTANCE`, choose a listed process ID. `PERMISSION_DENIED` gives each missing permission's `settingsPane`, the executable path, and whether it is ad-hoc signed. `IDENTITY_UNAVAILABLE` identifies the failed re-execution stage. `TIMEOUT`, `DISCOVERY_UNAVAILABLE`, and `CAPTURE_FAILED` are retryable. Other codes require changing arguments, application state, permissions, or installation conditions first. The schema index publishes every stable code and its exit status.

## Worked example

With `recon` on your path, learn the tree command without inspecting an application:

```sh
recon --help
recon ui tree --help
recon --version
recon schema
recon schema ui tree
recon context
```

Each succeeds on standard output. The first three print text; the remaining commands print one JSON object each. To inspect a usage error without consulting macOS:

```sh
recon ui tree Calculator --window abc
```

This exits 2, leaves standard output empty, and emits an `INVALID_ARGUMENT` object whose details identify `--window`, value `abc`, and the decimal window-ID requirement.

## Limits and edge cases

- The schema describes this binary, including its supported hosts. Capability commands still fail with `UNSUPPORTED_HOST` outside macOS; help, schema, and context work everywhere.
- `context` reads state only. Existing `ui` commands still request missing permissions before returning `PERMISSION_DENIED`.
- `app launch` can activate an application. `ui screenshot` writes a new file. Other commands are read-only; schema `effects` records these distinctions.
- `schema` reads nothing outside Recon and needs no permissions. Context probes the host but launches no application.
