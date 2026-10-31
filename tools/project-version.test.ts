import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

import { projectVersion, repoRoot, versionFrom } from "./project-version.ts";

test("reads the version out of package.json", () => {
    assert.equal(versionFrom('{"name":"example","version":"0.4.1"}'), "0.4.1");
});

test("rejects a package.json without a usable version", () => {
    assert.throws(() => versionFrom("{}"));
    assert.throws(() => versionFrom('{"version":""}'));
    assert.throws(() => versionFrom('{"version":1}'));
});

test("resolves the repository root from this file, not the process directory", () => {
    assert.equal(existsSync(join(repoRoot(), "package.json")), true);
    assert.match(projectVersion(), /^\d+\.\d+\.\d+/);
});
