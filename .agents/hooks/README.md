# Hooks

`.beads/hooks/commit-msg` holds commits made from Oh My Pi to Scoped Commits and lets every other commit through. It calls the validator in `.agents/hooks/check-scoped-commit`; CI calls the same validator for every commit in a pull request. Beads' other Git hooks live in `.beads/hooks/`, so both integrations use one hook directory.

`vp install` runs the package's `prepare` script, installing Vite+'s dispatcher in `.beads/hooks/_/`. This activates both the Beads hooks and the Scoped Commits hook without changing `core.hooksPath` between tools. To activate it without reinstalling dependencies:

```sh
vp hooks enable --hooks-dir .beads/hooks
```

`vp hooks status` shows whether it is active in a clone. CI runs the same validation on every pull request, so the hook is not the only line of defence.

| Lifecycle event                           | Run              |
| ----------------------------------------- | ---------------- |
| After a file edit                         | `vp fmt <file>`  |
| Before the agent declares a turn complete | `vp run check`   |
| Session start                             | Read `AGENTS.md` |

Harness lifecycle wiring is harness-specific and lives outside `.agents/`. For Oh My Pi it is `.omp/extensions/`.
