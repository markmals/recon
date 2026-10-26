# Recon

Recon aims to give coding agents a repeatable way to investigate applications and compare independent implementations with the originals, starting with native, Electron, and web applications on macOS. The [vision](VISION.md) describes the intended investigation loop. On macOS, the `recon` CLI can list application bundles, inspect their metadata and running process IDs, and launch a selected bundle.

## Investigate a macOS application

Build the executable with the pinned Rust toolchain, then install it somewhere on your `PATH`:

```sh
cargo build --release -p recon
mkdir -p ~/.local/bin
install -m 755 target/release/recon ~/.local/bin/recon
export PATH="$HOME/.local/bin:$PATH"
recon app list
recon app inspect /System/Applications/Calculator.app
recon app launch /System/Applications/Calculator.app
```

`list` and `inspect` leave foreground focus alone; `launch` can activate Calculator. For target selection, JSON fields, and discovery limits, see the [macOS application investigation guide](guides/macos-application-investigation.md). Pull requests also provide an arm64 macOS executable archive from the `macos-preview` CI job for previewing the current branch.

## Work on the project

Install [Vite+](https://viteplus.dev), [Beads](https://github.com/gastownhall/beads), and [Rustup](https://rustup.rs/) (which reads `rust-toolchain.toml`), then run:

```sh
vp install
vp run status
bd prime
vp run check
```

`vp run status` reports the current branch, pull request, proposal, and next process phase. `bd ready` lists tracked work. The [agent contract](AGENTS.md) and [development process](PROCESS.md) govern proposals, implementation, review, and authorization. A feature that chooses new behavior needs an approved proposal before code. The supplied brief informs the vision but is not an approval to implement every capability at once.

`vp run check` runs formatting, linting, TypeScript checks, artifact validation, tooling tests, the documentation build, and `vp run rust`. When a crate is added under `crates/`, register it in `Cargo.toml`: an unregistered crate fails the gate. Registered crates receive formatting, Clippy, test, and rustdoc checks, and Clippy and rustdoc warnings fail the gate. `vp run docs:dev` serves the project record locally; the docs preview workflow previews that record, while the `macos-preview` CI job supplies the executable.

The tooling tests build `recon` once in `target/tools` and use the executable path reported by Cargo, including when `CARGO_BUILD_TARGET` or `build.target` is configured. Their build does not replace the executable used by the Rust tests.

### IntelliSense

The repository's `.intellisense.jsonc` configures Rust, TypeScript/JavaScript, Vue, Swift, CSS, JSON/JSONC, YAML, TOML, Markdown, and shell language servers, including the extensionless Git hooks. Agents use the [using-intellisense skill](.agents/skills/using-intellisense/SKILL.md) for semantic queries and refactor previews.

Intellisense 0.2 or newer treats the `servers` map as the exact selection. The Rust, Swift, and TypeScript entries inherit preset filetypes; TypeScript overrides its preset's command to run the project's native LSP. The configuration file and repository boundary determine the workspace root, including invocations from `tools/`. Each selected server participates in workspace-wide commands when handled files are present, including the standalone Swift fixture and Markdown files without separate manifests.

The configuration references the checked-in `.intellisense.schema.json` for offline editor validation and completion. Refresh it after a CLI upgrade with `intellisense schema config | jq .result > .intellisense.schema.json`. The checked-in skill is synchronized from Intellisense's canonical `skills/using-intellisense/` directory.

Install the `intellisense` CLI and [Mise](https://mise.jdx.dev/), then install any missing servers:

```sh
mise use --global rust-analyzer@latest npm:bash-language-server@latest npm:vscode-langservers-extracted@latest npm:yaml-language-server@latest marksman@latest taplo@latest
mise install 'npm:@vue/language-server[allow_exotic_deps=@emmetio/css-parser]@2.2.12' npm:typescript@5.9.3
```

Globally selected Mise servers run through `mise exec`. If Rustup's `rust-analyzer` proxy is first on that environment's `PATH`, install the component for the active Mise Rust toolchain with `mise exec -- rustup component add rust-analyzer`. TypeScript uses the native LSP bundled with this project's TypeScript 7 dependency; run `vp install` first. Swift uses `xcrun sourcekit-lsp` from the Xcode command-line tools on macOS.

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

Pass `--trust-project` on server-backed commands after reviewing the configuration. Language-server diagnostics supplement `vp run check`. Shell, Vue, TOML, and YAML use pushed diagnostics, which cannot establish complete coverage; Markdown may provide no diagnostic reports. Read each server's coverage and limitations when `intellisense check` exits 3, even when no errors are observed.
