import assert from "node:assert/strict";
import { test } from "node:test";

import { findWorkspaceVersion, withWorkspaceVersion } from "./sync-version.ts";

const MANIFEST = `[workspace]
members = ["crates/*"]

[workspace.package]
version = "1.2.3"
edition = "2024"

[workspace.dependencies]
clap = { version = "4.5", features = ["derive"] }
`;

test("finds the version inside [workspace.package]", () => {
    assert.deepEqual(findWorkspaceVersion(MANIFEST), { line: 4, current: "1.2.3" });
});

test("ignores a dependency's version", () => {
    assert.equal(
        withWorkspaceVersion(MANIFEST, "2.0.0").includes('clap = { version = "4.5"'),
        true,
    );
});

test("ignores a version in a different table", () => {
    let manifest = `[package]
version = "9.9.9"

[workspace.package]
version = "1.2.3"
`;
    assert.deepEqual(findWorkspaceVersion(manifest), { line: 4, current: "1.2.3" });
});

test("rewrites only the workspace version", () => {
    let updated = withWorkspaceVersion(MANIFEST, "2.0.0");
    assert.equal(findWorkspaceVersion(updated).current, "2.0.0");
    assert.equal(updated.split("\n").length, MANIFEST.split("\n").length);
});

test("fails when the table or the line is missing", () => {
    assert.throws(() => findWorkspaceVersion("[workspace]\n"));
    assert.throws(() => findWorkspaceVersion('[workspace.package]\nedition = "2024"\n\n[other]\n'));
});
