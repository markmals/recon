---
id: proposal.0004
title: Selective Observation Output
authors: [Mark Malstrom]
status: draft
pull-request: https://github.com/markmals/recon/pull/35
issues: []
supersedes: []
---

# Selective Observation Output

## Summary

Let an agent ask for less: `recon app list --running`, `recon ui tree --depth`, and `recon ui tree --no-menu-bar`. Tree nodes stop spelling out absent attributes, and a result says whether it is short because the caller narrowed it or because Recon could not read everything.

## Authorization

- Authority: Mark consented on 2026-10-17, in conversation, to this proposal, after reviewing an analysis of Recon's CLI against agent-first CLI guidance. No build mandate applies.
- Fit: This proposal is limited to options that narrow `recon app list` and `recon ui tree` results, to the shape of a tree node, and to the vocabulary a result uses to say what it left out.
- Decisions recorded: None yet.

## Motivation

An agent asks Recon a question about a running application and receives an answer whose size is set by the application rather than by the question.

Observed on this Mac on 2026-10-17, under the conditions recorded in Domain grounding:

- `recon app list` reports 443 bundles in 64,913 bytes. Only 48 of them have a running process, and those 48 entries are 7,255 bytes. An agent asking what it can observe right now pays 65 KB for 7 KB of answer.
- `recon ui tree Safari` reports 6,051 nodes in 1,708,699 bytes. The menu bar is 3,097 of those nodes and 52% of the bytes.
- `recon ui tree MarkEdit` reports 406 nodes in 103,540 bytes, 405 of them the menu bar. The useful content of that result is that MarkEdit has no window on this Space.
- `recon ui tree Code`, an Electron application, reports 517 nodes in 133,220 bytes: 504 menu-bar nodes and a 12-node window.
- Every node names every attribute it does not have. In Notes' tree, 1,101 of 1,161 nodes carry `"description":null`, and 776 carry `"children":[]`.

The impact falls on the caller Recon is built for. An agent must either hold a megabyte of JSON in its context to find one button, or spend extra steps spilling it to a file and filtering it. Both pay for work Recon already did: reading Safari's menu bar costs roughly 6,200 accessibility requests whose results the caller discards, and reading a closed menu can make the application run its own menu-update code, which proposal 0002 recorded as an open uncertainty.

The current workaround is to run the full command and filter the output locally with a tool such as `jq`. It removes bytes from the agent's context but not the read from Recon's run, it requires the caller to buffer or spill the whole result first, and the filtered copy carries no record of what was dropped — so an observation kept as evidence (#23) cannot be told apart from a complete one.

## Domain grounding

Claims marked _observed_ come from measurements taken on 2026-10-17 on this Mac — macOS 27.0.1 (build 26A434), Apple silicon — using the Developer ID–signed `recon` at `target/signed/recon`, built and signed on 2026-10-15, against applications that were already running. Nothing was launched, activated, or quit to take them. Sizes are the bytes of the single JSON line Recon writes. Application versions: Safari 27.0.1 (22625.1.29.11.28), Notes 4.13 (3195), MarkEdit 1.36.0 (96), Finder 27.0 (1865), Code 1.142.0, Tailscale 1.104.1, Amphetamine 5.3.2 (9416). The strategist analysis that prompted this proposal was treated as inference: every number below was measured again here.

### Established context

**What an accessibility tree costs today**

| Application tree   | Nodes | Menu-bar nodes | Deepest node | Bytes     | Wall time   |
| ------------------ | ----- | -------------- | ------------ | --------- | ----------- |
| `ui tree Safari`   | 6,051 | 3,097          | 17           | 1,708,699 | 0.54–0.84 s |
| `ui tree Notes`    | 1,161 | 400            | 8            | 306,376   | 0.82–0.89 s |
| `ui tree Code`     | 517   | 504            | 8            | 133,220   | 0.38 s      |
| `ui tree MarkEdit` | 406   | 405            | 6            | 103,540   | 0.21 s      |
| `ui tree Finder`   | 402   | 392            | 6            | 103,040   | 0.22 s      |

Observed. None of these results was truncated, so each is the whole tree Recon can see. A command that reads no tree — `recon ui windows` or `recon app inspect` — takes 0.12–0.15 s on this Mac, so that is the fixed cost of resolving a target, re-executing for the privacy identity, and checking a permission; tree reading is what remains.

Observed: tree size is not stable over time where the application's content is not. Safari's tree measured 1,708,699 bytes and, minutes later, 1,104,842 bytes, as its page changed. Proposal 0002's grounding recorded Safari's application tree on 2026-10-15 as 3,158 nodes taking 2.9 seconds; the same command here read 6,051 nodes in under a second. Timing claims about accessibility reads are claims about one Mac on one day.

Observed: narrowing the same trees, by recomputing their JSON, is worth the following. Dropping the main menu bar from the root's children: Safari −51.9%, Notes −33.3%, Finder −97.4%, Code −97.8%, MarkEdit −99.7%. Omitting every `null` field and every empty array, which changes no information: Safari −17.8%, Finder −24.1%, MarkEdit −24.4%, Code −25.4%, Notes −27.8%. Both together: Safari −58.3%, Notes −53.1%. Limiting depth is the sharpest of the three: Notes' window tree is 760 nodes and 204,150 bytes whole, 35 nodes and 9,270 bytes at depth 3, and 6,815 bytes at depth 3 with absent fields omitted.

**Menu bars**

- Apple documents two menu-bar attributes on an application's accessibility element: [`menuBar`](https://developer.apple.com/documentation/appkit/nsaccessibility-swift.struct/attribute/menubar) ("The app's menu bar", `NSAccessibilityMenuBarAttribute`, `AXMenuBar`) and [`extrasMenuBar`](https://developer.apple.com/documentation/appkit/nsaccessibility-swift.struct/attribute/extrasmenubar) ("The app extras menu bar", `AXExtrasMenuBar`), the menu bar holding the application's status items.
- Observed: both appear as ordinary children of the application element. Amphetamine, Tailscale, 1Password, Velja, Pastebot, CodexBar, and WeatherMenu each report two children with role `AXMenuBar`: one spanning the screen at `(0, 0)`, 1352×29, and a small one at the right of the menu bar — for example `(910, 2.5)`, 24×24 — whose single child is an `AXMenuBarItem` with subrole `AXMenuExtra`. Nothing else distinguishes them: both have role `AXMenuBar`, no title, no subrole, and no identifier of their own.
- Observed: the status-item menu bar is not negligible. Its subtree held 132 nodes for Amphetamine, 138 for CodexBar, 60 for Tailscale, 39 for Velja, 9 for 1Password, and 2 each for Pastebot and WeatherMenu. For an application whose entire interface is a status item, that subtree is the interface.
- Observed: the position of the menu bar among the root's children varies by application. Finder listed `AXMenuBar` first and `AXScrollArea` second; Notes, Safari, and Code listed `AXWindow` first and `AXMenuBar` second; MarkEdit listed only `AXMenuBar`. An application element can also have no children at all: SystemUIServer reported none, and Dock's root reported no role and no children.
- A secondary report claims that from macOS 27 applications no longer expose their own `AXExtrasMenuBar`, because menu-bar rendering moved into a system agent ([pull request against mac-menubar-manager](https://github.com/milan-elsen/mac-menubar-manager/pull/8)); `MenuBarAgent` is indeed running on this Mac. The observation above contradicts that claim for what appears in `AXChildren`. Recon reads `AXChildren`, so what an application exposes there is what a tree contains, whoever draws it.
- Proposal 0002 established that menu items can be read without opening menus, that Recon never opens one, and that reading a closed menu can still make an application run its own menu-update code.

**Prior art for narrowing a UI tree**

- Chrome DevTools Protocol's [`Accessibility.getFullAXTree`](https://chromedevtools.github.io/devtools-protocol/tot/Accessibility/) takes an optional `depth`, "the maximum depth at which descendants of the root node should be retrieved", returning the full tree when it is omitted. Its `getPartialAXTree` narrows by naming a DOM node instead, which is identity-based retrieval that a macOS accessibility tree cannot offer.
- Playwright's [accessibility snapshot](https://playwright.dev/docs/api/class-accessibility) prunes "nodes that go unused on most platforms and by most screen readers" unless `interestingOnly` is set to false, and defaults to pruning. The whole class is deprecated in favor of ARIA snapshots. It is a semantic filter, not a structural bound.
- Appium's XCUITest and mac2 drivers expose [`snapshotMaxDepth`](https://appium.github.io/appium-xcuitest-driver/4.19/settings/), default 50, documented as a way "to prevent out of memory or timeout errors while getting the elements source tree" with the explicit tradeoff that "a part of the elements source tree might be lost". The Appium Inspector documentation names 62 as the maximum stable value. Depth limits are the conventional answer to an accessibility snapshot that is too large, and the convention includes saying when the limit cut something.
- [winctl](https://github.com/NSExceptional/winctl)'s `describe` takes `--depth N`, default 60, which bounds its walk, plus `--role`, `--title`, `--id`, and `--actionable` filters that are applied to each element after it has been read. Only the depth bound avoids work.
- Apple's [`AXUIElementCopyAttributeValues`](https://developer.apple.com/documentation/applicationservices/1462060-axuielementcopyattributevalues) reads an array attribute from an index for a maximum count, which is the supported way to page a node with very many children. Recon does not use it.
- Proposal 0002 established that accessibility elements have no durable identity: `AXIdentifier` is optional and assigned by the application's developer, and the private remote-token element numbers are per-process. No element in one result can be named in a later command.

### Relevant constraints and principles

- Proposal 0002's invariants bind this change: observation never activates the target, changes focus, moves the pointer, opens a menu, performs an action, or writes an attribute; traversal stays bounded; an incomplete result says so; an ambiguous instance or window is an error.
- Repository evidence: a tree result is `{"application":{...},"windowId":…,"truncated":…,"root":…}`, and `truncated` is the single field that tells a reader the tree is not the whole story. It is true when any node carries `childrenOmitted` or when a request timed out (`crates/recon/src/ui/tree.rs`). `childrenOmitted` is set when the depth cap of 64 is reached, when the 25,000-node budget runs out, when a timeout stops the walk, or when a child is not an accessibility element.
- Repository evidence: `valueTruncated` marks a text value cut at 4,096 scalars and deliberately does not make the tree `truncated`; a test asserts that "a cut value is not a truncated tree". `truncated` is about which elements were read, not about whether every byte of every value is present.
- Repository evidence: the tree reader in `crates/recon/src/ui/tree.rs` is source-agnostic and tested against fake element sources; the macOS adapter reads eleven attributes per element in one `AXUIElementCopyMultipleAttributeValues` call and the element's action names in one `AXUIElementCopyActionNames` call — two accessibility requests per node. Children arrive whole, inside that bulk read.
- Repository evidence: the node's always-present `null` fields are a serialization choice, not a domain fact. `valueTruncated` and `childrenOmitted` already appear only when true.
- Repository evidence: `recon app list` deduplicates by bundle path, sorts by name then path, reports `discovery: "indexed-and-running"`, and fails outright when indexed discovery is unavailable rather than returning a partial inventory (proposal 0001).
- Proposal 0001 rejected substring matching when _resolving_ a target, because choosing one application to inspect or launch from a loose match is an arbitrary selection. Listing is a different act: a filtered list selects nothing and the caller sees every match.
- Repository evidence: the flag grammar is strict. Flags may precede or follow the target; a repeated flag, an unknown flag, a flag a command does not accept, a missing or malformed value, and an extra positional are all usage errors; numeric values are plain ASCII digits. Today `app` commands accept no flags at all.
- [VISION.md](../VISION.md) requires that a claim stay traceable to the conditions that produced it, and that uncertainty stay explicit. A result that omits part of an application must say so in the result, not only in the command line that produced it.
- Proposal 0003 (https://github.com/markmals/recon/pull/34) covers help, usage-error specificity, structured errors and exit codes, and a machine-readable command schema. This proposal specifies behavior and output shape, not how a usage error is formatted.

### Quality bar

- A caller can ask for less without losing the ability to tell exactly what it did not get.
- A narrowed result never looks complete, and a complete result never looks narrowed. A reader that stores a result can answer "is this the whole tree?" from the result alone.
- Narrowing avoids work rather than hiding it: an excluded subtree costs no accessibility requests and cannot provoke the application's menu-update code.
- One node shape. Whatever a reader must handle is in the output, and the same observation means the same thing in every run.
- A change to what an unqualified command returns is a deliberate, recorded decision, not a side effect of adding an option.

### Remaining uncertainty

- Whether the element an application returns for its `AXMenuBar` attribute is always the full-width `AXMenuBar` child observed above. Recon reads no named attribute of the application root today, so this is inference from geometry and from the `AXMenuExtra` subrole of the other menu bar's child. The design keys exclusion on the attribute itself, which is the application's own answer, rather than on that inference.
- Whether an application that exposes a status item still answers `AXExtrasMenuBar` on macOS 27, given the secondary report above. Nothing in this proposal depends on that attribute.
- Whether reading a closed menu makes an application run menu-update code remains proposal 0002's open uncertainty. Excluding menus removes the question for callers who do not need them; it does not answer it.
- Measurements come from one Mac on one day. Applications with live content vary between runs, and only one Electron application was measured (#4 covers Electron accessibility).

## Existing baseline

Recon's own `ui tree` is the closest baseline, and most of it should be preserved. Its traversal is a single depth-first walk with hard caps, its reader is independent of macOS and tested against fake element sources, its per-node markers (`childrenOmitted`, `repeated`, failed nodes, `valueTruncated`) each name one specific fact, and `truncated` summarizes them in one place. Those properties are what make a narrowing option expressible at all: a new bound belongs in the generic reader, and a new marker joins a vocabulary that already exists.

Three parts of it are incidental rather than essential. Reading eleven attributes for every element in one bulk request is an efficiency choice, not a contract, but it does mean dropping fields from the output saves bytes without saving requests. Emitting every absent attribute as `null` is a serialization choice that the same file already contradicts for `valueTruncated` and `childrenOmitted`. And the depth cap of 64 exists as a safety limit against cyclic or pathological trees; it is not a caller-facing control, and today the result cannot distinguish a cut it made from any other.

winctl's `describe` is the closest external baseline. Its `--depth N` bound is worth adopting: it stops the walk rather than filtering its output. Its `--role`, `--title`, `--id`, and `--actionable` filters are not. They are applied after each element is read, so they spend every request anyway; they flatten the tree into a list of matches, losing the structure that makes a tree worth reading; and `--title` is a case-insensitive substring match, the kind of loose selection proposal 0001 rejected.

`recon app list` is the baseline for the list change. Its discovery, deduplication, sorting, and `discovery` field are sound and stay as they are; the gap is that it has no way to answer a narrower question.

## Proposed solution

Give the two commands whose results scale with the application a way to ask for less, and make every result say what it left out.

`recon app list --running` reports only bundles with a running process. `recon ui tree --depth <n>` stops the walk at a depth the caller chooses. `recon ui tree --no-menu-bar` leaves the application's menu bar unread, while keeping the menu bar that holds its status items. Nodes stop emitting attributes the element does not have. A result distinguishes the two reasons it can be short: `truncated` keeps its present meaning — Recon stopped short of something it tried to read — and a new `narrowed` says the caller's options kept Recon from reading part of the tree, with `depthLimited` and `menuBarExcluded` marking where.

```text
recon app list --running → the 48 bundles with a running process, not all 443
recon ui tree Safari --no-menu-bar → the windows, without the 3,097 menu nodes Recon never reads
recon ui tree Notes --window 17002 --depth 3 → 35 nodes instead of 760
recon ui tree Notes → the same facts in 27.8% fewer bytes
```

## Detailed design

### What a result says about its own completeness

- A tree result is **complete** when Recon read every accessibility element reachable from the root through `AXChildren`. An element that answered with a failure appears as a failed node in its place and does not make the result incomplete; the failure is visible where it happened.
- **Truncation** is involuntary incompleteness: Recon stopped short of reading something it set out to read — the depth cap, the node budget, a messaging timeout, or a child that is not an accessibility element. It is marked per node by `childrenOmitted` and summarized by `truncated`. Both keep exactly their present meaning: `truncated` is true when any node carries `childrenOmitted` or a timeout ended the read.
- **Omission** is requested incompleteness: an option told Recon not to read part of the tree. It is marked per node by `depthLimited` or `menuBarExcluded` and summarized by the new envelope field `narrowed`.
- `truncated: false` with `narrowed: false` means the result is complete. `truncated: true` invites a retry or a narrower request; `narrowed: true` invites a different request. One flag could not carry both meanings.
- `valueTruncated` keeps its separate job: it marks one cut text value and sets neither `truncated` nor `narrowed`, because every element was still read.
- A `repeated` node keeps its meaning: the element was already emitted in this result, so the result is complete without reading it twice.

### `recon app list [--running]`

- `--running` takes no value. `--running=yes`, a repeated `--running`, any other flag, and any positional argument are usage errors. `app inspect` and `app launch` do not accept it.
- With `--running`, `applications` holds exactly the entries whose `runningPids` is not empty. Entries keep the same fields, the same order (name, then path), and the same ascending `runningPids` as in the unfiltered list.
- `discovery` is `"running"` when the flag is given and `"indexed-and-running"` when it is not. The field continues to name the boundary of the list, so a stored result says which question it answers.
- Discovery itself is unchanged. Both sources are still consulted, a running application whose bundle is not indexed is still listed with the metadata read from its bundle, and unavailable indexed discovery is still an error rather than a partial list.
- When no application has a running process, the result is an empty `applications` array, not an error.

### `recon ui tree <target> [--pid <pid>] [--window <id>] [--depth <n>] [--no-menu-bar]`

**`--depth <n>`**

- `<n>` is one or more ASCII decimal digits, parsed as `--pid` and `--window` values are, and must be between 0 and 64 inclusive. `0` is allowed and returns the root alone. A value above 64 is a usage error naming 64 as the maximum, because 64 is the depth cap Recon never reads past and a higher limit would promise depth that does not exist.
- The root is at depth 0. Recon reads the attributes and action names of a node at depth `<n>` and does not read its children.
- A node at depth `<n>` whose `AXChildren` has at least one member carries `"depthLimited": true` and no children. A node at that depth with no children carries no marker, because nothing was omitted. At that depth Recon does not examine the members of `AXChildren`, so a member that is not an accessibility element is part of the requested omission rather than a `childrenOmitted`.
- `childrenOmitted` is not set for a cut the caller asked for, and `depthLimited` is not set for a cut Recon made on its own. Without `--depth`, the depth cap of 64 behaves exactly as it does today: `childrenOmitted` and `truncated`.
- The node budget of 25,000 and the two-second messaging timeout are unchanged and still apply within the chosen depth; a tree can be both narrowed and truncated.
- `--depth` applies the same way to an application-level tree and to a `--window` tree.

**`--no-menu-bar`**

- `--no-menu-bar` takes no value, may appear once, and is accepted only by `ui tree`, and only without `--window`: a window tree contains no menu bar, so combining them is a usage error that says so rather than a flag that quietly does nothing.
- Before traversal, Recon reads the application element's `AXMenuBar` attribute — one accessibility request.
    - When it answers with an accessibility element, that element is excluded. While listing the root's children, Recon skips the child equal to it: no request is made for that element or anything below it, it is not counted against the node budget, and nothing stands in its place among the root's children. The root node carries `"menuBarExcluded": true` and the result's `narrowed` is `true`.
    - Two elements are equal here by the same identity Recon already uses to recognize a repeated element.
    - When the attribute is absent, unsupported, or answers with something that is not an accessibility element, nothing is excluded, no marker appears, and `narrowed` stays false for this reason: the application has no menu bar to exclude.
    - When that request does not complete within the messaging timeout, the command fails with the existing `process <pid> did not answer an accessibility request within 2 seconds`, rather than returning a tree that still contains the menu bar the caller excluded.
- Only the root's direct children are compared against the menu bar element; the comparison is not repeated deeper in the walk.
- A menu bar holding status items is a different element from the one `AXMenuBar` names, so it stays in the tree, as does every window. Excluding the menu bar never removes anything else.
- When the depth limit keeps Recon from listing the root's children at all — `--depth 0` — nothing is skipped on the menu bar's account: Recon does not read the `AXMenuBar` attribute, and the root carries only `depthLimited`. Otherwise the two options combine, and the root can carry both markers.
- Without the flag, the menu bar is read exactly as it is today.

### Node shape

- A node omits every field whose value would be `null` and every array that would be empty. `role`, `subrole`, `title`, `description`, `value`, `identifier`, `enabled`, `focused`, `frame`, `actions`, and `children` appear only when the element has them. An absent field means precisely what `null` and `[]` mean today: the element does not expose that attribute, exposes a value that is not a string, number, or boolean, advertises no action, or has no children.
- The boolean markers continue to appear only when true: `valueTruncated`, `childrenOmitted`, and the new `depthLimited` and `menuBarExcluded`.
- A repeated node is `{"role": <role>, "repeated": true}`, or `{"repeated": true}` when the element had no role. A failed node is unchanged: `{"error":"element-unavailable"}`, `{"error":"timeout"}`, or `{"error":"read-failed","axError":<code>}`.
- A node that has no attributes, no actions, and no children is `{}`. It says what it says today: the element answered, and exposed nothing.
- The result envelope keeps every field at all times: `{"application":{"path":…,"pid":…},"windowId":<id or null>,"truncated":<bool>,"narrowed":<bool>,"root":<node>}`. The envelope appears once per result and names the observation; nodes repeat thousands of times, which is the only place where absent fields cost anything.
- `app list`, `app inspect`, `ui windows`, and `ui screenshot` keep their current output, including their `null` fields.

### `narrowed`

- `narrowed` is `true` when any node in the result carries `depthLimited` or `menuBarExcluded`, and `false` otherwise.
- It describes the result, not the request. `--depth 64` on a tree six levels deep, and `--no-menu-bar` on an application with no menu bar, both leave `narrowed` false, because nothing was left unread.

### Errors and other hosts

- Every new option follows the existing grammar: it may appear once, a command that does not take it rejects it, and a missing or malformed value is a usage error. Under the CLI contract in force today that means the usage block on standard error and a nonzero exit with no success JSON. If proposal 0003 (https://github.com/markmals/recon/pull/34) is accepted first, these become its structured invalid-argument errors, and these options appear in its command schema.
- Behavior on hosts other than macOS is unchanged.

## Compatibility

Recon has no external users, and no stored data to migrate. Two changes alter what an existing command returns without being asked for, and both are deliberate.

Tree nodes stop emitting absent fields. A reader that treats a missing field as unavailable is unaffected; a reader that indexes every field unconditionally must change. The affected readers inside the repository are the byte-exact output tests in `crates/recon/tests/ui_response.rs`, the live assertions in `crates/recon/tests/ui_observation.rs`, and the node description in `guides/macos-ui-observation.md`.

Tree results gain `narrowed`. A reader that checks `truncated` to decide whether a tree is complete keeps the answer it has today for every unnarrowed command, and needs `narrowed` as well once it passes `--depth` or `--no-menu-bar`.

`recon app list` without `--running` is unchanged, including `discovery: "indexed-and-running"`. `ui windows`, `ui screenshot`, `app inspect`, and `app launch` are untouched.

## Implications on adoption

- No new permission, no new signing requirement, and no change to how Recon is installed or granted. `--no-menu-bar` adds one accessibility request before traversal and removes every request below the menu bar.
- An agent that wants narrower results passes the new options. The node-shape change reaches every `ui tree` caller at once, which is why it is recorded here rather than hidden behind an option.
- Adoption is reversible in the sense that matters: the options are optional, and a caller that passes none gets the same information it gets today, in the smaller node encoding.

## Scope

- `recon app list --running`, including the `discovery` value `"running"`.
- `recon ui tree --depth <n>` and `recon ui tree --no-menu-bar`, including their usage errors and their interaction with `--window`, the node budget, and the messaging timeout.
- The node encoding change, the `narrowed` envelope field, and the `depthLimited` and `menuBarExcluded` node markers.
- Tests:
    - CI tests, independent of macOS permissions: the flag grammar for all three options; depth-limited nodes and their markers; exclusion of a nominated element by identity, with the remaining children unaffected; `narrowed` and `truncated` set independently of each other; the node encoding, including a node that serializes to `{}`; `app list --running` filtering, ordering, and `discovery` value.
    - macOS integration tests against the fixture application: an application tree with and without `--no-menu-bar`, asserting that the Fixture menu disappears, that `menuBarExcluded` and `narrowed` are set, and that the fixture's windows remain; `--depth 0` and `--depth 1` shapes; an unchanged frontmost application and pointer.
- Guide updates: the node shape, the completeness vocabulary, and the new options in `guides/macos-ui-observation.md`, and `--running` in `guides/macos-application-investigation.md`.

### Out of scope

- Help, version, usage-error specificity, structured error payloads, exit codes, the command registry, and the machine-readable schema. Those belong to proposal 0003 (https://github.com/markmals/recon/pull/34).
- Field projection — letting a caller choose which node fields to receive. It is the largest remaining lever and is left to Future directions.
- A caller-chosen node budget, which narrows a tree at an arbitrary frontier rather than at a meaningful boundary. Future directions records why.
- Retrieving a subtree by naming an element from an earlier result. Accessibility elements have no durable identity: `AXIdentifier` is optional and assigned by the application's developer, the private remote-token element numbers are per-process, and nothing in a tree is guaranteed to denote the same element in a later run. `--window` stays the only sub-application root, and a window ID itself lasts only for the login session.
- Changes to `ui windows`. Its results are small — 938 bytes for the seven windows Notes owns — and grounding found no need.
- Interaction (#24), feature discovery (#17), the evidence store (#23), and Electron accessibility (#4).

## Preview

- Artifact: the `macos-preview` CI artifact for this pull request, `recon-macos-arm64`, containing the ad-hoc–signed `recon` executable and the built `ReconFixture.app`. The reviewer re-signs `recon` locally with their Developer ID Application identity as described in [the UI observation guide](../guides/macos-ui-observation.md), installs it at the path they have granted, and runs the narrowed commands against the fixture launched with `open -g`: `ui tree` with and without `--no-menu-bar`, `--depth 0`, `--depth 1`, and the combination, checking the markers and that the fixture's windows survive menu-bar exclusion. They then run the same commands against a real running application with many menus, and `app list` with and without `--running`, comparing sizes while another application stays frontmost.
- Reason: narrowing is only observable against a real accessibility tree read by a permissioned executable. The fixture makes the menu-bar and depth behavior checkable against known content, and a real application shows whether the sizes move as measured. Rendered documentation cannot exercise either.

## Policies and decisions checked

- [decision.0001](../decisions/0001-macos-privacy-identity.md) — honored and unchanged. Narrowing adds no permission and no new privacy surface: `--no-menu-bar` issues one extra accessibility request after the self-disclaiming re-execution, under Recon's own grant, and `app list --running` needs no permission at all. Reading less never requests more.
- `policies/` contains only a README; no policy applies.

## Future directions

- Field projection. Observed: a Notes tree carrying only `role`, `title`, and `children` is 37,274 bytes, 83% below the same tree with absent fields omitted, and Safari's is 341,683 bytes. It needs a vocabulary for field sets and a way to describe a node shape that varies per call, which fits better after the machine-readable schema in proposal 0003 (https://github.com/markmals/recon/pull/34); the fields it would drop — `frame`, `actions`, `value` — are the ones interaction (#24) depends on.
- A caller-chosen node budget. A depth-first budget stops at wherever the walk happens to be, so the shape of the result depends on the order an application lists its children, and two runs can cut in different places. A depth limit narrows predictably instead.
- Ranged child reads with `AXUIElementCopyAttributeValues`, which could bound a single node with very many children, rather than bounding the tree.
- Filtering `app list` by activation policy. Observed: of the 48 bundles with a running process, 37 declare `LSUIElement` or `LSBackgroundOnly`, leaving 11 ordinary applications; `NSRunningApplication.activationPolicy` is the authoritative source for that distinction.
- Answering `app list --running` without indexed discovery, since running applications come from the workspace rather than the index. That would relax proposal 0001's rule that unavailable indexed discovery is an error, and deserves grounding in how often the index is actually unavailable.
- Semantic pruning in the spirit of Playwright's `interestingOnly`, which would need a defensible definition of an uninteresting macOS element.
- A menu-only tree, the inverse of `--no-menu-bar`, if feature discovery (#17) wants the menu catalogue without the windows.

## Alternatives considered

- **Filtering after reading, as winctl's `describe` does.** Rejected. It spends the same accessibility requests, so it saves bytes but not time and does not avoid touching menus, and winctl's version flattens the tree into a list of matches.
- **Excluding the menu bar by role or by position.** Rejected on observation: role `AXMenuBar` also matches the menu bar holding an application's status items, which is the whole interface of several applications measured here, and the menu bar's position among the root's children varies by application. The application's own `AXMenuBar` attribute is the only non-arbitrary answer to which element is the menu bar.
- **`--menu-bar include|exclude` instead of a boolean flag.** It records the intent in a stored command whichever way the default goes, and it needs no new grammar beyond the existing value flags. Rejected as extra ceremony for a two-valued choice, but it is the natural form if the default ever flips.
- **Redefining `truncated` to cover requested omissions, or replacing both with a single `complete`.** Rejected. One flag cannot distinguish "a retry may give more" from "you asked for less", and redefining `truncated` would silently change what every result already stored or already documented means.
- **Echoing the effective options in the result envelope.** Rejected. The caller knows its own arguments; what it cannot know is whether they changed the result. `narrowed` and the node markers report that, and nothing else needs echoing.
- **Making `--depth 1` mean the root alone.** Rejected. Recon already says the root is at depth 0, and the depth cap of 64 is expressed the same way; a second counting convention in the same command would be a trap.
- **Substring or pattern filtering for `app list`.** Rejected for now, though proposal 0001's rejection of substring matching does not forbid it: a filtered list selects nothing and shows every match. It would need its own matching semantics — case, Unicode normalization, which fields participate — and that is a second vocabulary beside proposal 0001's exact resolution rules. `--running` answers the question that was actually observed, and an exact name or identifier already answers "where is this application".
- **Keeping the node encoding and adding `--compact`.** Rejected in the recommendation below: two node shapes would have to be handled by every consumer, every test, and every stored observation, to avoid a change that no external caller would notice.

## Open questions

- [NEEDS CLARIFICATION: Should `recon ui tree` keep reading the application's menu bar by default? Options: (a) keep today's default and add `--no-menu-bar`, so an unqualified `ui tree` still means "the whole tree" and feature discovery (#17) keeps the menu catalogue it will rely on; (b) exclude the menu bar by default and spell the flag `--menu-bar` to include it, which would drop 52% of Safari's bytes, 33% of Notes', 97% of Code's, and 97–99% of Finder's and MarkEdit's from the common case, at the cost of making `recon ui tree MarkEdit` return a nearly empty tree unless the caller knows the flag. Recommendation: (a).]
- [NEEDS CLARIFICATION: Should the smaller node encoding be the only encoding? Options: (a) always omit absent fields and empty arrays — one node shape, no information lost, 17.8–27.8% fewer bytes in the trees measured here; (b) keep today's encoding and add an option for the smaller one, so no existing reader changes, at the price of two node shapes in every consumer, test, and stored observation; (c) leave the encoding as it is and rely on the other options. Recommendation: (a), with the guide stating plainly that an absent field means the element does not expose that attribute.]
