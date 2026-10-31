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

The release workflow opens and updates the "Version Packages" pull request as a GitHub App rather than with the workflow's own `GITHUB_TOKEN`, because GitHub holds CI on a pull request that `GITHUB_TOKEN` created until someone approves the runs. The app needs only the repository permissions Contents (read and write) and Pull requests (read and write), no webhook, and an installation on this repository. Its client ID is the repository variable `RELEASE_APP_CLIENT_ID`, and its private key is the repository secret `RELEASE_APP_PRIVATE_KEY`. Without both, the `Version` job fails and nothing can be released.
