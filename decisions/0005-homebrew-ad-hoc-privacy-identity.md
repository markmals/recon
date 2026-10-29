---
id: decision.0005
title: Homebrew Builds Keep Recon's Ad-Hoc Privacy Identity
status: accepted
established-by: user.2026-10-29
supersedes: [decision.0002]
---

# Homebrew Builds Keep Recon's Ad-Hoc Privacy Identity

## Summary

Recon retains its executable-owned `me.malstrom.recon` privacy identity. Homebrew source and bottle builds remain ad-hoc signed, so users grant Accessibility and Screen Recording to each installed build; local Developer ID signing remains optional for preserving grants across in-place development rebuilds.

## Context

Decision 0002 established that privacy grants belong to Recon rather than its launcher, and deferred CI signing until a distribution channel existed. Homebrew now supplies that channel, but its formula and bottle builds do not have Mark's Developer ID private key. Putting that credential into the personal tap's CI would broaden access to a signing identity for every formula build, while a Developer-ID-signed prebuilt archive would bypass the requested plain source-formula and bottle workflow. Recon still needs a stable bundle identifier, observable permission state, and an explicit account of when grants survive replacement.

## Decision

Recon's privacy identifier remains `me.malstrom.recon`. The bare executable embeds that `CFBundleIdentifier` and the name `recon` in its `__TEXT,__info_plist` section. Commands whose behavior or report depends on Recon's own grants re-execute once with the private responsibility-disclaim spawn attribute. They never use the launcher's grants as a fallback. Permission requests and typed failures retain decision 0002's behavior; `recon context` reports identity, signing, and preflight permission state without requesting a grant.

Homebrew builds and bottles use the ad-hoc signatures produced by the build. The formula does not request a Developer ID credential, notarize a separate archive, or alter TCC state. Users grant Screen Recording and Accessibility to the installed `recon` executable and repeat the grants when an upgrade or reinstall changes its ad-hoc code hash or versioned Cellar path. `recon context` is the non-interactive way to inspect the current installation's grants and signing facts.

A developer may still sign a stable local executable with a Developer ID Application identity and the identifier `me.malstrom.recon`, using `RECON_SIGNING_IDENTITY=… vp run recon:sign`, to preserve grants across in-place rebuilds at that path. This optional development arrangement is not a requirement for Homebrew installation.

## Rationale

The existing privacy model depends on a consistent executable identity, not on a particular signing certificate. Ad-hoc Homebrew builds preserve that model and the requested bottle supply chain without distributing a private key or maintaining a second binary artifact. Making re-granting explicit avoids presenting an ad-hoc bottle as though it had the upgrade continuity of a Developer-ID-signed installation.

## Alternatives considered

- Store a Developer ID private key in tap CI — grants a repository-wide release workflow access to a long-lived signing credential solely to preserve local privacy grants.
- Publish a separately signed and optionally notarized binary archive — preserves grants more effectively but replaces the requested source-built formula and CI bottle path with a second release system.
- Keep requiring every user to sign Recon locally with Developer ID — excludes users without a paid signing identity and makes the canonical installation harder without improving observation behavior.
- Return privacy grants to the launching terminal — avoids Recon-owned signing concerns but makes results depend on the launcher, which decision 0002 rejected.

## Consequences

### Good

- Homebrew installation needs no signing certificate or new credential in CI.
- Recon keeps the same bundle identifier, responsible-process behavior, permission errors, and read-only `context` report across installation methods.
- The formula and bottles exercise the same native artifacts users receive.
- Developers who need stable grants across repeated local builds retain the existing Developer ID signing task.

### Bad

- Homebrew upgrades and replaced bottles can require fresh Accessibility and Screen Recording grants.
- Moving between a Homebrew installation and a development copy creates separate privacy clients and grant state.
- Recon continues to depend on private responsibility-disclaim and code-signing-query interfaces.
- macOS 15 and later can still periodically request Screen Recording reconfirmation independently of Recon's release mechanism.

## Revision history

| Date | Change | Authority |
| --- | --- | --- |
| 2026-10-29 | Preserved Recon's privacy identity while accepting ad-hoc Homebrew builds and their re-granting cost. | user.2026-10-29 |
