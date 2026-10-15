# Recon

Recon aims to give coding agents a repeatable way to investigate desktop applications and compare independent implementations with the originals. The [vision](VISION.md) describes the intended investigation loop. This repository currently contains the development workbench and project record; it does not yet contain a Recon CLI or application adapters.

## Work on the project

Install [Vite+](https://viteplus.dev), [Beads](https://github.com/gastownhall/beads), and [Rustup](https://rustup.rs/) (which reads `rust-toolchain.toml`), then run:

```sh
vp install
vp run status
bd prime
vp run check
```

`vp run status` reports the current branch, pull request, proposal, and next process phase. `bd ready` lists tracked work. The [agent contract](AGENTS.md) and [development process](PROCESS.md) govern proposals, implementation, review, and authorization. A feature that chooses new behavior needs an approved proposal before code. The supplied brief informs the vision but is not an approval to implement every capability at once.

`vp run check` runs formatting, linting, TypeScript checks, artifact validation, tooling tests, the documentation build, and `vp run rust`. The Rust task explicitly skips crate checks while the workspace has no members. When a crate is added under `crates/`, register it in `Cargo.toml`: an unregistered crate fails the gate. Registered crates receive formatting, Clippy, and test checks. `vp run docs:dev` serves the project record locally; the docs preview workflow previews that record, not an installable CLI.
