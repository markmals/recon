# Recon

Recon aims to give coding agents a repeatable way to investigate applications and compare independent implementations with the originals, starting with native, Electron, and web applications on macOS. The [vision](VISION.md) describes the intended investigation loop. On macOS, the `recon` CLI can list application bundles, inspect their metadata and running process IDs, and launch a selected bundle.

## Install Recon

Install the official release from the personal Homebrew tap:

```sh
brew install markmals/tap/recon
```

Homebrew installs the CLI and its matched native inspection artifacts. Subsequent upgrades use `brew upgrade recon`.

## Investigate a macOS application

```sh
recon app list
recon app inspect /System/Applications/Calculator.app
recon app launch /System/Applications/Calculator.app
```

`list` and `inspect` leave foreground focus alone; `launch` can activate Calculator. For target selection, JSON fields, and discovery limits, see the [macOS application investigation guide](guides/macos-application-investigation.md). Pull requests also provide an arm64 macOS executable archive from the `macos-preview` CI job for previewing the current branch.

## Work on the project

Install [Mise](https://mise.jdx.dev), which provisions every other pinned tool — Rust, Node, pnpm, Beads, hk, Fnox — then run:

```sh
mise install
mise run setup
mise run status
mise run check
```

`mise install` installs the toolchains pinned in `mise.toml`; `mise run setup` installs dependencies, initializes the work tracker, and installs the git hooks. Both are idempotent, and `setup` is needed only once per clone. A Homebrew source build reads `rust-toolchain.toml` through Rustup instead; the two pin the same Rust version.

`mise run status` reports the current branch, pull request, proposal, and next process phase. `bd ready` lists tracked work. The [agent contract](AGENTS.md) and [development process](PROCESS.md) govern proposals, implementation, review, and approval. A feature that chooses new behavior needs an approved proposal before code. The supplied brief informs the vision but is not an approval to implement every capability at once.

`mise run check` is the gate: formatting, linting, type checks, crate membership, the tooling and Rust test suites, rustdoc, record validation, and the documentation build. When a crate is added under `crates/`, register it in `Cargo.toml`; an unregistered crate fails `mise run crates`. Clippy and rustdoc warnings fail the gate too. `mise run docs:dev` serves the project record locally; the docs preview workflow previews that record, while the `macos-preview` CI job supplies the executable.

`mise tasks` lists everything, including the native inspection tasks — `fixture:build`, `helper:build`, `loader:build`, `recon:sign`, `test:ui`, `test:focus`, `test:inject`, `test:helper`, `test:inspection`. They drive a live Mac, so they run only on macOS and stay out of `mise run check`.

The tooling tests build `recon` once in `target/tools` and use the executable path reported by Cargo, including when `CARGO_BUILD_TARGET` or `build.target` is configured. Their build does not replace the executable used by the Rust tests.

The indexed-discovery and foreground-preservation integration tests use a 20-second deadline because they read the live macOS application catalogue.

### IntelliSense

The repository's `.intellisense.jsonc` configures Rust, TypeScript/JavaScript, Vue, Swift, CSS, JSON/JSONC, YAML, TOML, and Markdown language servers. Agents use the [using-intellisense skill](.agents/skills/using-intellisense/SKILL.md) for semantic queries and refactor previews.

Intellisense 0.2 or newer treats the `servers` map as the exact selection. The Rust, Swift, and TypeScript entries inherit preset filetypes; TypeScript overrides its preset's command to run the project's native LSP. The configuration file and repository boundary determine the workspace root, including invocations from `tools/`. Each selected server participates in workspace-wide commands when handled files are present, including the standalone Swift fixture and Markdown files without separate manifests.

The configuration references the checked-in `.intellisense.schema.json` for offline editor validation and completion. Refresh it after a CLI upgrade with `intellisense schema config | jq .result > .intellisense.schema.json`. The checked-in skill is synchronized from Intellisense's canonical `skills/using-intellisense/` directory.

Install the `intellisense` CLI, then install any missing servers:

```sh
mise use --global rust-analyzer@latest npm:vscode-langservers-extracted@latest npm:yaml-language-server@latest marksman@latest taplo@latest
mise install 'npm:@vue/language-server[allow_exotic_deps=@emmetio/css-parser]@2.2.12' npm:typescript@5.9.3
```

Globally selected Mise servers run through `mise exec`. If Rustup's `rust-analyzer` proxy is first on that environment's `PATH`, install the component for the active Mise Rust toolchain with `mise exec -- rustup component add rust-analyzer`. TypeScript uses the native LSP bundled with this project's TypeScript 7 dependency; run `mise run setup` first. Swift uses `xcrun sourcekit-lsp` from the Xcode command-line tools on macOS.

Vue uses the separately installed 2.2.12 server in standalone mode with a Mise-managed TypeScript 5.9.3 SDK. The launcher in `tools/vue-language-server.ts` runs with Node, resolves both installations through Mise, and initializes Vue's standalone service. Newer Vue servers require a custom TypeScript request bridge, and TypeScript 7 no longer provides the JavaScript SDK that standalone Vue needs. The globally selected Vue and TypeScript versions remain unchanged.

The Vue install allows only its `@emmetio/css-parser` dependency from the upstream `ramya-rao-a/css-parser#vscode` GitHub fork. Mise's default installer otherwise blocks this non-registry dependency; other dependencies retain that check.

Review the project commands before trusting them:

```sh
intellisense config show
intellisense config validate
intellisense --trust-project server list
intellisense --trust-project capabilities --file crates/recon/src/lib.rs
intellisense --trust-project find buildFixture --file tools/build-fixture.ts
intellisense --trust-project capabilities
intellisense --trust-project find Recon
```

Pass `--trust-project` on server-backed commands after reviewing the configuration. Language-server diagnostics supplement `mise run check`. Vue, TOML, and YAML use pushed diagnostics, which cannot establish complete coverage; Markdown may provide no diagnostic reports. Read each server's coverage and limitations when `intellisense check` exits 3, even when no errors are observed.
