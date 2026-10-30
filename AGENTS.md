# Agent Contract

This project uses the **Workbench** development process. A human approves intent in a proposal; agents implement it as tests, guides, and code; an independent review checks them against each other before the human reviews the result. [`PROCESS.md`](PROCESS.md) describes the whole loop and its artifacts for humans. The skills named below hold the procedures.

Run `mise run status` at the start of every session and whenever you are unsure where the work stands. It reads the branch, the pull request, and the active proposal, and prints the phase and the next step. The human is never expected to remember a command or skill name; "keep going" or a plain description of a problem is enough to orient from it.

## Does this change need a proposal?

Ask one question: **does this change decide something?**

- If intent already exists — an approved proposal promises the behavior, or the correct behavior is obvious (a typo, a dead link, a bug whose fix is unambiguous) — implement it directly. Say which case it is, in one line, before you start.
- If the change decides something, ask the human whether they want a proposal for it. Only after an explicit yes: create a branch, push it, open a draft pull request, and write the proposal with `.agents/skills/writing-a-proposal/`. A request to "start a feature" or "write a proposal" is not that yes, and neither is `mise run status` output.
- If a small fix turns out to need a design decision, stop and ask the proposal question then.

Proposal status belongs to the human. They move a proposal from `draft` to `awaiting-implementation`, and from `active-review` to `accepted`, `returned-for-revisions`, or `rejected`. You move it to `active-review` when you hand finished work to them, and to `implemented` in the accepted pull request before merge (`.agents/skills/completing-a-feature/`).

## Implementing an approved proposal

Use `.agents/skills/implementing-a-proposal/`. In short:

1. Read the proposal's Detailed design and the code it touches.
2. Write failing tests for the behavior Detailed design specifies, and watch them fail for the right reason.
3. When the change alters what a user does, sees, or must know, write or extend the guide in `guides/` from the proposal, not from the code, and check it against the tests.
4. Write the code.
5. Verify with `mise run check`. Report the commands you ran and what they printed.
6. Get one independent review with `.agents/skills/reviewing-an-implementation/` and fix what it confirms. Then post the readiness report, move the proposal to `active-review`, and mark the pull request ready, together.

Which of tests, guides, and code a change needs is a judgement; the order is not. A change with no user-facing surface needs no guide, and a change with nothing meaningfully testable needs no test, but say so rather than skipping quietly.

Work inline by default. Use subagents only when they buy something specific: slices that are independent, touch separate files, and save real time by running in parallel, or a fresh-context reviewer that has not seen your reasoning. A fixed batch of independent slices goes to the harness's `task` tool; for stages that depend on each other, finish the prerequisite and hand its verified result to the next agent.

## Rules

- **Stay inside the proposal.** Behavior it does not describe does not get built; if it should exist, revise the proposal with the human. A defect outside its scope becomes a Beads item ([Work tracking](#work-tracking), `.agents/skills/tracking-work/`); a defect that stops the proposal from working is your work.
- **Raise ambiguity.** When the proposal, vision, a policy, or a decision is ambiguous or conflicts with the work in a way that changes behavior, stop and raise it rather than choosing silently.
- **Never write production code before its failing test**, when the behavior warrants a test at all.
- **Never write a guide from finished code.**
- **Never claim work is done without running the verifying command this turn** and reading its output ([`.agents/rules/verification.md`](.agents/rules/verification.md)).
- **Find a bug's root cause before fixing it** ([`.agents/skills/systematic-debugging/`](.agents/skills/systematic-debugging/)).
- **The adversarial reviewer reports; it never edits.**
- **Never delete a branch, force-push, publish a release, or deploy without explicit human confirmation.**
- **Record a policy or decision only after discussing it with the human.** Most features establish neither; a record belongs there only when its knowledge outlives the change.

## Quality gates

```sh
mise run setup       # idempotent first-run bootstrap after a clone
mise run check       # the single blocking gate; CI and the pre-push hook both call it
mise run fmt         # format in place; accepts file paths
mise run lint        # lint, applying the safe fixes
mise run typecheck   # type-check both TypeScript projects
mise run test        # the tools' tests and the Rust test suite
mise run doc         # rustdoc over the workspace, warnings denied
mise run build       # build every target in the workspace
mise run crates      # every directory under crates/ is a workspace member
mise run validate    # record frontmatter, IDs, and cross-references
mise run docs:build  # the documentation site builds
mise run status      # phase, next action, who acts
mise run changeset   # add a changeset for this change
```

`mise run check` is the gate: formatting, lint, types, crate membership, tests, rustdoc, the record rules, and the documentation site, in one task. Nothing is done until it passes in this turn. `mise tasks` lists everything available, including the native inspection tasks the gate deliberately leaves out ([Platform](#platform-macos-inspection-cli)).

## Work tracking

Beads (`bd`) is the work tracker, the ready-work queue, and durable agent memory. It replaces GitHub-issue defect tracking, markdown TODO lists, and MEMORY files. Proposals and Beads answer different questions: the proposal is the record of **intent**, reviewed by a human and preserved as history; Beads is the **work graph** — what is ready, what is blocked, what was discovered and deferred, and what a future session needs to remember.

```sh
bd ready                 # what can be worked on now
bd show <id>             # read one item before touching it
bd update <id> --claim   # claim atomically before starting
bd create "Title" --description="why this exists" --type=task --priority=2
bd close <id> --reason="Completed"
bd remember "<fact>"     # durable project memory
```

Rules that matter:

- Never run `bd edit`; it opens an interactive editor and will hang a non-interactive agent. Use `bd update` flags.
- Priorities are `0`–`4`, `0` highest. Default `2`. Do not invent a scale.
- Claim before starting and close only when the work is actually done.
- Never run `bd hooks install`; hk owns the git hooks and runs `bd hooks run <hook>` as a step.
- `.beads/issues.jsonl` is a passive export that is committed; the database is the source of truth.

A defect that stops the current implementation from satisfying its proposal is **not** a new work item. It is the current work, and it gets fixed. Skill: [`.agents/skills/tracking-work/`](.agents/skills/tracking-work/).

## Tooling

**Mise** owns toolchains, environment, and the task graph. Every command an agent runs routinely is a `mise run <task>`; `mise install` provisions the pinned toolchains from `mise.toml` and `mise.lock`. Rust is pinned twice on purpose: `mise.toml` for everything that goes through mise, and `rust-toolchain.toml` for rustup, which is what the Homebrew formula's source build and rust-analyzer use. Add a task rather than documenting a bare command: a task is an enforceable tier, prose is not.

**hk** owns every git hook. `mise run setup` installs them with `hk install --mise`. Never install hooks any other way, and never bypass them with `--no-verify`. The `commit-msg` hook holds every commit to [Scoped Commits](https://scopedcommits.com/) with `tools/check-scoped-commit.ts`, whose allowed scopes live in `.agents/commit-scopes`; `pre-push` runs the gate.

**Fnox** owns secrets. A task that needs a secret declares it with `secrets = [...]` in `mise.toml`, and Fnox resolves it from 1Password or the macOS Keychain at task time. Never put a secret in `[env]`, in a file, or in a commit. The code-signing identity `mise run recon:sign` reads is not a secret: it names a certificate already in the developer's keychain.

**Beads (`bd`)** owns the work graph and agent memory. See [Work tracking](#work-tracking).

**Changesets** owns the changelog and versioning. `mise run changeset` adds one; `mise run version` applies it and syncs the Cargo workspace version. Every pull request carries a changeset.

**Worktrunk (`wt`)** owns worktrees. `wt switch --create <name>` creates a parallel worktree that trusts and installs its own toolchain and copies the ignored files listed in `.worktreeinclude`. Run parallel feature work in a worktree rather than stashing.

**GitHub Actions** runs the same `mise run check` the pre-push hook runs. If it passes locally and fails in CI, the difference is environmental — find it, do not special-case the workflow.

TypeScript is covered by Oxfmt, Oxlint, and `tsrs`; Rust by `cargo fmt`, Clippy, and rustdoc. There is no bundler and no Bun: Node runs `tools/*.ts` directly through native type stripping.

## Platform: macOS inspection CLI

A macOS command-line program that investigates running applications for an agent. One Cargo workspace with one binary, plus two native artifacts Cargo does not build — an Objective-C helper that runs inside the inspected process and a C loader that gets it there. It ships through a Homebrew tap.

| Piece | Where |
| --- | --- |
| Workspace metadata, shared version, lint policy | `Cargo.toml` — `members = ["crates/recon"]` |
| The binary | `crates/recon/src/main.rs` — parse, render the error, exit |
| Everything the binary does | `crates/recon/src/lib.rs` and the modules beneath it: `application`, `cli`, `context`, `error`, `inspection`, `macos`, `output`, `request`, `ui` |
| Tests that need a process, a fixture, or a live Mac | `crates/recon/tests/` |
| Embedded `Info.plist` and build revision | `crates/recon/build.rs` — macOS records a privacy grant against the executable's signing identity, so the bare binary carries a plist |
| Injected helper, Objective-C | `helper/` — `ReconCore`, `ReconInspect`, `ReconServer`, `ReconFacets`, `ReconFields`, with its own tests in `helper/tests/` |
| Injector, C | `loader/` — `recon_loader.c` and the entitlements it is signed with |
| Fixture application | `crates/recon/tests/fixtures/ReconFixture` — a Swift AppKit app the native tasks build into `target/fixtures/` |
| Documentation site | `docs/` — VitePress; `docs/.vitepress/config.ts` renders the records from the repository root |
| Repository tooling | `tools/*.ts` — TypeScript run directly by Node, no bundler |
| Formatting | `rustfmt.toml` and `.oxfmtrc.jsonc` — the same 100 columns and 4 spaces on both sides |
| Toolchain | `mise.toml` for everything that goes through mise; `rust-toolchain.toml` for rustup |

### Commands

| Command | What it does |
| --- | --- |
| `mise run dev -- app list` | Run the CLI from source |
| `mise run build` | `cargo build --workspace --all-targets` |
| `mise run build:release` | The optimized `recon` a release ships |
| `mise run test` | The `tools/` tests and `cargo test --workspace` |
| `mise run fmt [paths…]` | Oxfmt and rustfmt; with no arguments, the whole tree |
| `mise run lint` | `oxlint --fix` and `cargo clippy --workspace --all-targets -- -D warnings` |
| `mise run doc` | `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS=-D warnings` |
| `mise run crates` | Fails when a directory under `crates/` is not a workspace member |
| `mise run docs:dev` · `docs:build` · `docs:preview` | The documentation site |

`lint`, `test`, and `doc` are the house Rust verification set, alongside the formatting check `mise run check` runs. Never substitute a command that checks a weaker property, and never report a check as passing unless it ran. Use `cargo` directly for anything without a task (`cargo add`, `cargo tree`, `cargo test -p recon --test cli_contract`).

### Native inspection tasks

These build the native artifacts and exercise them against a live Mac — the signing keychain, a running fixture application, an injected process, the accessibility permission grant:

| Command | What it does |
| --- | --- |
| `mise run helper:build` · `loader:build` | Compile the helper dylib and the loader with the Xcode command-line tools |
| `mise run fixture:build` · `fixture:inject` | Build the fixture app bundle, and the injectable variant |
| `mise run recon:sign` | Code-sign the debug `recon` with `RECON_SIGNING_IDENTITY` |
| `mise run test:ui` · `test:focus` | UI observation against the live fixture, and the evidence that reading never steals focus |
| `mise run test:helper` · `test:inject` · `test:inspection` | The helper's own tests, loader injection, and live inspection end to end |

They observe a Mac rather than a repository, so they are **not** part of `mise run check` and cannot run in the Linux CI gate. Run them on a macOS host when a change touches the native path, and report what they printed. On any other platform they refuse to run rather than pretending to pass.

### Hard rules

- **Never `#[allow]`.** Silence a lint on the item with `#[expect(…, reason = "…")]`, which fails the build once the lint stops firing.
- **Never hand-edit the version in `crates/recon/Cargo.toml`, `package.json`, or `CHANGELOG.md`.** Changesets owns them; `mise run version` writes the Cargo side, and `node tools/sync-version.ts --check` fails CI when they drift.
- **Every directory under `crates/` is a workspace member.** Add it to `Cargo.toml` in the same change; `mise run crates` fails the gate on an unregistered crate.
- **Keep the two Rust pins equal**: `rust` in `mise.toml` and `channel` in `rust-toolchain.toml`. Nothing checks the pair, and a mismatch surfaces first in someone else's `brew install`.
- `Cargo.lock` is committed and is part of the release contract. Dependency changes belong in their own changeset.

## Harness setup

The process lives harness-neutral in `AGENTS.md` and `.agents/`, which stay the source of truth and which no harness forks. Lifecycle wiring is the only harness-specific part, and this repository ships it for [Oh My Pi](https://omp.sh):

| What | How Oh My Pi gets it |
| --- | --- |
| This contract | Loads `AGENTS.md` from the repository root |
| Skills | Discovers `.agents/skills/` |
| Before the first turn | `.omp/extensions/prime-beads.ts` runs `bd prime` and injects the work graph and durable memories into the session |
| After every edit | `.omp/extensions/format-after-edit.ts` runs `mise run fmt -- <files>` on each file the `edit` or `write` tool changed and reports a reformat or a parse failure |
| Before the turn ends | `.omp/extensions/check-before-stop.ts` is present but disabled on purpose |

The pre-stop gate is intentionally inert — its own comment records the decision — so running `mise run check` and reading its output is the agent's own job, every turn, before claiming anything is done. A harness without extensions loses only the convenience, not the rule: `bd prime` and `mise run check` are commands the agent runs itself, and hk's `pre-push` hook and CI run the gate regardless. Another harness wires the same events into its own configuration and commits the result rather than forking `.agents/`.

Git hooks belong to hk, not to a harness: `mise run setup` installs them, and they run for every commit regardless of who or what made it.

## Also binding

- [`.agents/rules/code-quality.md`](.agents/rules/code-quality.md) — what good code looks like here.
- [`.agents/rules/commit-discipline.md`](.agents/rules/commit-discipline.md) — commits, staging, push and PR policy.
- [`.agents/rules/enforcement-hierarchy.md`](.agents/rules/enforcement-hierarchy.md) — where a new rule belongs.

### Cross-cutting reasoning skills

- [`.agents/skills/grounding-a-problem/`](.agents/skills/grounding-a-problem/) — learn the problem domain before deciding on a solution.
- [`.agents/skills/ontology-review/`](.agents/skills/ontology-review/) — review the conceptual model when important domain concepts change.
- [`.agents/skills/reframing-a-problem/`](.agents/skills/reframing-a-problem/) — challenge the problem formulation when the current model stops explaining reality.
- [`.agents/skills/systematic-debugging/`](.agents/skills/systematic-debugging/) — find root cause when the intended behavior is sound but implementation is wrong.
- [`.agents/skills/using-intellisense/`](.agents/skills/using-intellisense/) — ask the language servers for symbols, references, and diagnostics instead of grepping.
