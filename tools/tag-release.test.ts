import assert from "node:assert/strict";
import { test } from "node:test";

import { decide } from "./tag-release.ts";

const ENTRY = "### Patch Changes\n\n- Fix the menu-bar filter.";

test("releases a new version that has no tag yet", () => {
    let decision = decide("1.4.0", false, ENTRY);
    assert.equal(decision.tag, "v1.4.0");
    assert.equal(decision.release, true);
});

test("does not release when the tag already exists", () => {
    // The usual case: a push that opened or updated the Version Packages pull
    // request, rather than one that merged it.
    assert.equal(decide("1.4.0", true, ENTRY).release, false);
});

test("releases every version once, including a first release", () => {
    // Recon tags by hand today; the workflow must reach the same `v<version>` tag.
    assert.deepEqual(decide("0.1.2", false, ENTRY), {
        tag: "v0.1.2",
        release: true,
        reason: "Tagging v0.1.2.",
    });
});

test("refuses to release a version that has no changelog entry", () => {
    // A hand-edited version without a hand-written entry must not reach a tag.
    assert.throws(
        () => decide("0.1.3", false, undefined),
        /CHANGELOG\.md has no entry for 0\.1\.3/,
    );
});

test("needs no changelog entry when nothing is being released", () => {
    assert.equal(decide("0.1.2", true, undefined).release, false);
});
