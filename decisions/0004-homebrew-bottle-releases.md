---
id: decision.0004
title: Homebrew Bottle Releases
status: accepted
established-by: user.2026-10-29
supersedes: []
---

# Homebrew Bottle Releases

## Summary

Recon publishes versioned GitHub releases and a plain formula in `markmals/homebrew-tap`. The formula builds and bottles the CLI with its matched native inspection artifacts, making Homebrew the canonical local installation and upgrade path.

## Context

Recon is a macOS CLI, but native inspection also needs a loader, a universal Apple-silicon helper, and the helper's build identity beside the executable. Installing only `target/release/recon` leaves those operations unavailable. The personal tap already builds plain formulae and publishes CI-built bottles through `brew test-bot` and `brew pr-pull`; a cask or a manually copied binary would bypass that established release path.

## Decision

Tag each official Recon release as `v<version>`, matching the `recon` crate version, and create a GitHub release for that tag. Distribute Recon through `markmals/homebrew-tap` as `Formula/recon.rb`, not as a cask.

The formula builds from the release's Git tag and pinned revision with the Rust and Apple toolchains. It builds the CLI, loader, and helper; installs `recon`, `recon-loader`, `recon-helper.dylib`, and `recon-helper.build-id` together in `libexec`; and exposes only the CLI through a `bin` exec script. Tap pull requests use the existing macOS bottle matrix and formula tests. The `pr-pull` workflow publishes those bottles and updates the formula rather than checking bottle binaries into either repository.

Homebrew is the canonical local installation. Release upgrades update the crate version, tag and GitHub release first, then update the formula's tag and revision and repeat bottle validation and publication.

## Rationale

A source-built formula preserves Recon's native build requirements while bottles make normal installation fast and repeatable. Keeping all four artifacts in one versioned prefix preserves the executable-adjacent lookup and helper build compatibility contract. Reusing the tap's existing automation avoids a second release system and provides the same review and bottle-publication controls as the other CLI formulae.

## Alternatives considered

- A Homebrew cask containing a manually assembled binary archive — unsuitable for a CLI and duplicates the tap's existing bottle infrastructure.
- Installing only the Rust executable — leaves native inspection without its required loader, helper, and build identity.
- Checking prebuilt binaries into the Recon repository or tap — obscures provenance and bypasses per-macOS-version bottle builds.
- A Recon-owned release workflow that duplicates bottle publication — adds automation without replacing any missing capability in the tap's existing `test-bot`/`pr-pull` flow.

## Consequences

### Good

- `brew install markmals/tap/recon` installs every artifact needed by the released CLI.
- Homebrew owns subsequent local upgrades and removal.
- Bottle provenance and macOS compatibility are checked through pull request CI.
- The CLI remains the only command exposed on `PATH`; native implementation artifacts stay in `libexec`.

### Bad

- A release spans the Recon and tap repositories and must be tagged before the tap formula can fetch its source.
- Each release requires formula tag and revision updates and new bottles for the supported macOS runners.
- Source builds require Rust, Node, and Apple build tools even though ordinary bottle installations do not compile Recon.
- Homebrew's ad-hoc-signed installation does not replace the separate privacy grants or research-host requirements of native inspection.

## Revision history

| Date | Change | Authority |
| --- | --- | --- |
| 2026-10-29 | Accepted GitHub releases and the bottled Homebrew formula as Recon's distribution path. | user.2026-10-29 |
