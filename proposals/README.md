# Proposals

This directory holds the durable design record for one change per file.

Name each file `<NNNN>-<slug>.md`, using the next four-digit number and a kebab-case slug. Required frontmatter is `id: proposal.<NNNN>`, `title`, `authors` as a non-empty list, `status`, `pull-request`, and `supersedes`. Optional `issues` is a list of issue references or URLs. The normal lifecycle is `draft` → `awaiting-implementation` → `active-review` → `accepted` → `implemented`.

| Status | Meaning | Decided by | Phase |
| --- | --- | --- | --- |
| `draft` | Being written and iterated on in the draft pull request. May carry `[NEEDS CLARIFICATION:`. | agent | 2 |
| `awaiting-implementation` | The approval owner settled the proposal. Implementation is underway or not yet started; the feature pull request becomes ready once the complete implementation is verified and pushed. | approval owner | 3 |
| `active-review` | The readiness report is posted after Phase 3 reviews and preview; the pull request was made ready earlier. | agent | 4 |
| `returned-for-revisions` | The acceptance review asked for changes. Back to implementation, or to the proposal if the design changed. | approval owner | 4 → 3 or 2 |
| `accepted` | The approval owner accepted the work. Vision update, merge and required delivery are underway. | approval owner | 5 |
| `implemented` | The approved implementation and final records are committed in the feature pull request before merge. Merge and any required release remain separate delivery events. | agent | 5 |
| `rejected` | The approval owner declined the change. Kept as record. | approval owner | terminal |
| `withdrawn` | The author pulled it. Kept as record. | either | terminal |
| `superseded` | A later proposal replaced it. | either | terminal |

Pull-request readiness and proposal status track different facts. A ready feature PR exposes implemented code for inspection while its proposal may still be `awaiting-implementation`; the `active-review` status records the later readiness-report handoff to acceptance review.

**Decided by** names the approval owner: the human, or the strategist for decisions delegated by [an active build mandate](https://github.com/markmals/recon/blob/main/AGENTS.md#bounded-build-mandates). Proposal approval requires settled intent and recorded authority. Acceptance of implementation additionally requires the readiness report, fresh checks, independent reviews, and an exercised preview. An implementer's completion claim alone cannot authorize acceptance.

Every proposal records where its authority came from in `## Authorization`: the human's direct consent for this change, or the mandate proposal and approved revision it runs under, why the change fits inside it, and the approval decisions the strategist took. A proposal may also carry an optional `## Build mandate` section; once the human approves that proposal, it is what delegates a bounded set of decisions. Mandates need no separate directory, frontmatter key, or status — an ordinary proposal in this lifecycle carries them — and a mandate proposal cannot authorize itself.

After acceptance and the vision update, the agent records `implemented` in the same feature pull request before merging it. This status does not assert that the pull request has merged or that a required release happened. The pull request records integration; release records and the task tracker keep any outstanding delivery visible until it is separately authorized and performed. A merge-only change needs no release.

A proposal stays `draft` for as long as it takes. It is published to the draft pull request as soon as there is something to react to, and the approval owner iterates on it there — editing directly, or asking for changes — until the design is settled. Iteration is the normal case, not a sign of a bad first draft.

History is preserved: never delete a superseded entry — mark it `superseded` and link its replacement through `supersedes`.
