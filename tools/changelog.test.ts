import assert from "node:assert/strict";
import { test } from "node:test";

import { changelogEntry } from "./changelog.ts";

// The shape `changeset version` writes with @changesets/changelog-github.
const CHANGELOG = `# recon

## 0.2.0

### Minor Changes

- [#81](https://github.com/markmals/recon/pull/81) Thanks [@markmals](https://github.com/markmals)! - Add \`ui view layers\`.

## 0.1.30

### Patch Changes

- Report a renamed bundle by its display name.

## 0.1.3

### Patch Changes

- Fix the menu-bar filter.
`;

test("returns the entry under the version's heading and nothing from the next version", () => {
    let entry = changelogEntry(CHANGELOG, "0.2.0");
    assert.equal(
        entry,
        "### Minor Changes\n\n- [#81](https://github.com/markmals/recon/pull/81) Thanks [@markmals](https://github.com/markmals)! - Add `ui view layers`.",
    );
});

test("reads the last entry up to the end of the file", () => {
    assert.equal(
        changelogEntry(CHANGELOG, "0.1.3"),
        "### Patch Changes\n\n- Fix the menu-bar filter.",
    );
});

test("does not take a version whose number only starts with the requested one", () => {
    assert.equal(
        changelogEntry(CHANGELOG, "0.1.3"),
        "### Patch Changes\n\n- Fix the menu-bar filter.",
        "0.1.30 is a different release",
    );
    assert.equal(changelogEntry(CHANGELOG, "0.1"), undefined);
});

test("reports no entry for a version the changelog never mentions", () => {
    assert.equal(changelogEntry(CHANGELOG, "0.1.4"), undefined);
});

test("reports no entry for a heading with nothing under it", () => {
    let changelog = "# recon\n\n## 0.1.4\n\n   \n\n## 0.1.3\n\n- Fix the menu-bar filter.\n";
    assert.equal(changelogEntry(changelog, "0.1.4"), undefined);
});

test("does not read a version mentioned in prose as its heading", () => {
    let changelog = "# recon\n\n## 0.1.3\n\n- Supersedes the 0.1.4 draft.\n";
    assert.equal(changelogEntry(changelog, "0.1.4"), undefined);
});

test("reports no entry when the version has only subheadings and no changes", () => {
    let changelog =
        "# recon\n\n## 0.1.4\n\n### Patch Changes\n\n## 0.1.3\n\n- Fix the menu-bar filter.\n";
    assert.equal(changelogEntry(changelog, "0.1.4"), undefined);
});

// Changesets indents a summary's continuation lines under its list item, but a
// hand-written entry may put a fence at the start of a line.
test("keeps a shell comment inside a fenced example as part of the entry", () => {
    let entry =
        "### Minor Changes\n\n- Add `ui view layers`:\n\n```sh\n# one window\nrecon ui view layers Finder\n```";
    let changelog = `# recon\n\n## 0.2.0\n\n${entry}\n\n## 0.1.3\n\n- Fix the menu-bar filter.\n`;
    assert.equal(changelogEntry(changelog, "0.2.0"), entry);
});

test("does not take a heading inside a fenced example as the version's entry", () => {
    let entry = "### Patch Changes\n\n- Document the format:\n\n~~~md\n## 0.1.3\n~~~";
    let changelog = `# recon\n\n## 0.2.0\n\n${entry}\n\n## 0.1.3\n\n- Fix the menu-bar filter.\n`;
    assert.equal(changelogEntry(changelog, "0.2.0"), entry);
    assert.equal(changelogEntry(changelog, "0.1.3"), "- Fix the menu-bar filter.");
});
