---
name: using-intellisense
description: Use when finding symbol definitions or usages, assessing callers and implementations, checking LSP diagnostics or baselines after edits, planning semantic renames and code actions with intellisense, or deciding which servers and workspace root it should use.
---

# Using Intellisense

Use `intellisense` for semantic evidence. Edit files normally; queries synchronize disk changes. Keep project tests as independent verification.

## Start in a project

Run from the project directory, or pass `--workspace /absolute/project`. Root selection reads no configuration key and no language manifest: an explicit `--workspace` wins, otherwise the nearest `.intellisense.jsonc` within the closest repository boundary, then the repository root, then the starting directory. To make a subdirectory the root, pass `--workspace` or put an `.intellisense.jsonc` in it. `Cargo.toml`, `package.json` and `Package.swift` do not move the root.

A configured, installed server participates in workspace-wide commands when the workspace holds files it handles, after exclusions. Loose `.swift` files select `sourcekit-lsp` without `Package.swift`. The language server discovers its own compile database, module graph or build plan; selection does not guarantee complete semantic support for standalone files.

Run `intellisense server list`, then `intellisense capabilities --file FILE`. Disambiguate with `--server NAME`. Missing servers/runtimes are prerequisites; report them. Intellisense does not install servers.

## Choose which servers run

Layers are built-in presets, the user file, then the project `.intellisense.jsonc` (or `--config PATH`). `intellisense config show` prints the merged result with its sources; `intellisense schema config` is authoritative for key names. There is no `enabled` flag, no `extends`/`override` key, no single-file toggle, and no root setting.

`servers` is a selection, not a patch:

| Goal | Configuration |
| --- | --- |
| Inherit the current selection | omit `servers`; with no explicit map in any layer, this selects all presets |
| Only rust-analyzer | `"servers": {"rust-analyzer": {}}` |
| No servers at all | `"servers": {}` |
| Adjust one preset | list it with just the changed keys: `"servers": {"gopls": {"priority": 10}}` |
| Replace a preset, inheriting nothing | leave its name out; add your own name with `command` and `filetypes` |
| Drop one preset | list the names you keep |

A listed preset name starts from its built-in `command`/`filetypes` plus any user-file override of that name, then takes the keys written in this layer. `filetypes` maps extension or file name to LSP language id (`{"swift": "swift"}`); it is not a list. A `null` server entry, a `null` `servers` map, and `rootMarkers` are rejected by validation.

A custom server without `filetypes` is available through explicit `--server NAME`, but cannot be selected automatically. Refresh a checked-in editor schema after upgrading with `intellisense schema config | jq .result > .intellisense.schema.json`; `init` preserves existing files.

Project changes to `command`, `settings`, or `initializationOptions` that were not already authorized by presets or user configuration require `--trust-project`, a global flag accepted anywhere in the command line. Selecting presets by name, filetypes, priority, excludes, limits and timeouts do not. Review `intellisense config show` before authorizing; never use `--trust-project` just to dismiss errors. Configuration is part of session identity, so regenerate handles and plans after a change and check baseline comparability.

## Discover exact arguments

```sh
intellisense cli search "find callers"
intellisense schema impact
intellisense docs search "baseline"
```

`COMMAND --help` is text. JSON results use stdout; operational errors use stderr. Capture both streams and exit status.

## Quick reference

Prefix these commands with `intellisense`:

| Need | Command |
| --- | --- |
| Symbols | `find NAME --file FILE --limit 10` |
| Contract and usages | `inspect --at FILE:LINE:COLUMN --references --implementations --examples 2` |
| Static relationships | `impact --symbol HANDLE` |
| Current diagnostics | `check` or `check --file FILE` |
| Baseline comparison | `check baseline save NAME`, later `check --baseline NAME` |
| Rename preview | `refactor rename --at FILE:LINE:COLUMN --to NAME` |
| Available transformations | `refactor actions --at FILE:LINE:COLUMN` |
| Server troubleshooting | `server status`, `server restart`, `server stop` |

Positions use 1-based Unicode scalar columns. Resolve ambiguity explicitly. `verified: false` entries lack handles; handles expire after source/session changes. References/examples are opt-in. Query callable symbols for call hierarchy.

## Example investigation and rename

Example: `MemoryCache` at `cache.go:3:6`, as returned by `find`:

```sh
intellisense find MemoryCache --file cache.go --limit 5
intellisense inspect --at cache.go:3:6 --references --examples 2
intellisense check baseline save before-cache-change
intellisense refactor rename --at cache.go:3:6 --to MemoryStore
```

Review the preview, then explicitly run `intellisense refactor apply PLAN_ID` with the returned identifier. Plans survive separate invocations in the same live workspace/configuration/server session. Stale or expired plans require a new preview, even if the edit range is unchanged. Check `applied`/`matchesPreview`, run `intellisense check --baseline before-cache-change`, then project tests.

## Interpret evidence

Read `status`, `limitations`, coverage, section `supported`, and truncation. Partial results can contain real errors.

- Exit 0: operation succeeded; semantic sections can still be partial. Saving a baseline confirms persistence, not cleanliness.
- Exit 1: operational failure.
- Exit 2: complete check found errors, or newly observed errors against a comparable baseline.
- Exit 3: incomplete/unavailable verification, potentially with observed errors. Inspect diagnostics and counts.

## Common mistakes

Unsupported means unknown, not absent. Push-only zero-error checks are unverified. Baselines preserve that uncertainty; claim deltas only when comparable, with matching scope/configuration. References are not exhaustive dependencies. Hover is textual server evidence. Command-only actions/resource operations cannot be applied.

Report friction with version, server, command, and status/limitations; distinguish missing LSP support from CLI bugs.
