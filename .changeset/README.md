# Changesets

Every pull request, and every commit pushed straight to `main`, adds one file here describing what it changes for the people who use Recon. [Changesets](https://changesets.dev) turns those files into version bumps and `CHANGELOG.md` entries when the "Version Packages" pull request is merged.

```sh
mise run changeset              # pick the bump (patch / minor / major) and write the summary
mise run changeset -- --empty   # nothing releasable changed; say so explicitly
mise run version                # apply pending changesets locally (CI normally does this)
```

Write the summary for a reader of the changelog, not for the reviewer of the diff: what they can now do, what no longer works, what they must change. Recon's audience is the person who runs `brew install markmals/tap/recon` and then points the CLI at a running application — so a change to the harness, the record tooling, or the gate is an empty changeset, and a change to a command, its output, or its privacy requirements is not.

Recon's version lives in the private root `package.json`, which is never published to npm; `privatePackages.version` in `config.json` is what makes Changesets bump it anyway. The crate carries the same number: `mise run version` runs `tools/sync-version.ts`, which copies it into `[workspace.package] version` and refreshes `Cargo.lock`. Never edit either by hand — `node tools/sync-version.ts --check` fails CI when they drift.

Merging the "Version Packages" pull request pushes the `v<version>` tag and publishes the GitHub Release for it. That release is what `markmals/homebrew-tap` builds and bottles `Formula/recon.rb` from, so updating the formula's tag and revision is the deliberate next step in the tap repository (decision.0004).

Every release needs a `## <version>` entry in `CHANGELOG.md`. The release workflow refuses to tag a version without one, and the GitHub Release's notes are that entry followed by GitHub's list of merged pull requests. Changesets writes the entry in the "Version Packages" pull request. If a version is ever bumped by hand instead, write its entry by hand in the same pull request, under a `## <version>` heading.
