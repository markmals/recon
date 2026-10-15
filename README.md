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

`vp run check` runs formatting, linting, TypeScript checks, artifact validation, tooling tests, the documentation build, and `vp run rust`. When a crate is added under `crates/`, register it in `Cargo.toml`: an unregistered crate fails the gate. Registered crates receive formatting, Clippy, and test checks. `vp run docs:dev` serves the project record locally; the docs preview workflow previews that record, while the `macos-preview` CI job supplies the executable.
