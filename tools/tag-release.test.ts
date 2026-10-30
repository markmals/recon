import assert from "node:assert/strict";
import { test } from "node:test";

import { decide } from "./tag-release.ts";

test("releases a new version that has no tag yet", () => {
    let decision = decide("1.4.0", false);
    assert.equal(decision.tag, "v1.4.0");
    assert.equal(decision.release, true);
});

test("does not release when the tag already exists", () => {
    // The usual case: a push that opened or updated the Version Packages pull
    // request, rather than one that merged it.
    assert.equal(decide("1.4.0", true).release, false);
});

test("releases every version once, including a first release", () => {
    // Recon tags by hand today; the workflow must reach the same `v<version>` tag.
    assert.deepEqual(decide("0.1.2", false), {
        tag: "v0.1.2",
        release: true,
        reason: "Tagging v0.1.2.",
    });
});
