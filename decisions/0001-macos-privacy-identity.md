---
id: decision.0001
title: Recon Owns Its macOS Privacy Identity
status: accepted
established-by: proposal.0002
supersedes: []
---

# Recon Owns Its macOS Privacy Identity

## Summary

macOS privacy grants belong to the `recon` executable itself, identified as `me.malstrom.recon`. Recon embeds that identity, re-executes itself as its own responsible process before any permission-gated work, and is signed locally with a Developer ID identity so grants survive rebuilds.

## Context

Screen Recording and Accessibility are privacy permissions that macOS (TCC) attributes to a _responsible process_. For a command-line tool, that is the terminal, editor, or agent harness that launched it. Recon is run by many launchers. If it inherited their permissions, the same command would work under one harness and fail under another, and its evidence would not be reproducible.

macOS records a grant for a bare executable against its path and its code-signing designated requirement. The Rust toolchain ad-hoc signs builds, and an ad-hoc designated requirement is the code hash, so macOS forgets the grant on every rebuild. A Developer ID signature's requirement names the signing identifier and the team instead.

Proposal 0002 grounded these facts on macOS 27.0.1 with SIP enabled. Re-executing with the private `responsibility_spawnattrs_setdisclaim` spawn attribute made a test program responsible for itself. A bare executable with an embedded Info.plist appeared in System Settings under its bundle name. Its grant survived a rebuild signed with the same identity and installed at the same path, but a copy at another path had no grant. Wrapping the same program in an app bundle produced no permission prompt.

Every later feature that reads or drives another application's UI needs the same permissions, starting with UI interaction (#24). Changing the identifier later would invalidate every user's grants, which is why it is recorded here rather than left in one proposal.

## Decision

- Recon's privacy identifier is `me.malstrom.recon`, under Mark's `me.malstrom.*` namespace for `malstrom.me`. The `recon` executable embeds an Info.plist in its `__TEXT,__info_plist` section with that `CFBundleIdentifier` and the `CFBundleName` `recon`, and its code-signing identifier is the same string.
- Before doing anything that needs a privacy permission, a command makes `recon` responsible for itself. If macOS reports another process as responsible, `recon` re-executes itself once with the disclaim spawn attribute and passes on its arguments, environment, standard streams, termination signals, and exit status. If that is impossible, the command fails rather than running under the launcher's permissions. Commands that need no permission do not re-execute.
- Recon checks a permission without prompting, asks macOS to request it when it is missing, and then fails with an error. The error names the permission, the executable's path, and the System Settings pane. Recon never writes to the TCC database or works around a missing grant.
- `recon` ships as a bare executable, not an application bundle. Users sign it locally with a Developer ID Application identity and the identifier `me.malstrom.recon` (`codesign --force --sign "<identity>" --identifier me.malstrom.recon <path>`). Contributors use `RECON_SIGNING_IDENTITY=… vp run recon:sign`. CI builds stay ad-hoc signed until Recon has a distribution channel, such as a Homebrew formula, that brings signing into CI. That change revises this decision.

## Rationale

Owning the identity is the only arrangement under which a grant means the same thing in every terminal and agent harness. Recon's evidence depends on observations that can be repeated, so a grant that varies with the launcher would undermine every result built on it.

An embedded Info.plist gives the bare executable a recognizable name in System Settings without the packaging and launch semantics of an app bundle. The bare form is also the one verified to prompt and keep its grant. A Developer ID signature is the cheapest signature whose designated requirement survives a rebuild. Local signing keeps a certificate out of CI until a distribution channel needs one.

The identifier follows the owner's existing reverse-DNS convention, because any later change costs every user a fresh grant.

## Alternatives considered

- **Use the launcher's permissions.** This needs no private interface, but grants differ between terminals and harnesses, and it would grant Recon whatever the launcher happens to hold.
- **Wrap `recon` in an app bundle.** This could make grants independent of the install path, but the one attempt produced no permission prompt. It remains a possible revision alongside notarized distribution.
- **Ship ad-hoc signed builds only.** This needs no certificate, but macOS forgets the grant on every rebuild or upgrade.
- **Sign pull-request and release builds in CI now.** This produces ready-to-grant downloads, but places a signing certificate in CI before there is a distribution channel. It was deferred until a Homebrew formula exists.
- **Use `com.markmals.recon`.** It was rejected in favor of the owner's established `me.malstrom.*` namespace.

## Consequences

### Good

- A permission grant behaves the same whichever program launches `recon`, including cron jobs and agent harnesses.
- One grant per installed executable survives in-place upgrades signed with the same identifier and team.
- Interaction (#24) and later UI features inherit the identity, re-execution, and permission errors instead of solving them again.
- A missing permission is reported with an actionable path and pane, never hidden behind empty or partial results.

### Bad

- Recon depends on the private `responsibility_get_pid_responsible_for_pid` and `responsibility_spawnattrs_setdisclaim` interfaces. A macOS update can remove them, and Recon then reports the capability as unavailable.
- Grants are tied to a path. Moving the executable, or running a second copy elsewhere, needs a new grant.
- Keeping grants across rebuilds requires a Developer ID certificate. Without one, a user must grant again after every build.
- CI downloads must be re-signed before use until CI signing exists.
- The identifier must never change casually. Superseding it invalidates every existing grant.
- Since macOS 15, macOS periodically asks users to reconfirm screen-recording permission, which can interrupt unattended use.

## Revision history

| Date       | Change                  | Proposal      |
| ---------- | ----------------------- | ------------- |
| 2026-10-15 | Accepted this decision. | proposal.0002 |
