import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test, type TestContext } from "node:test";
import { fileURLToPath } from "node:url";

let checkCrates = fileURLToPath(new URL("./check-crates.ts", import.meta.url));

/** A throwaway workspace whose `members` list holds exactly `members`. */
function workspace(t: TestContext, members: string[]): string {
    let root = mkdtempSync(path.join(tmpdir(), "recon-crates-"));
    t.after(() => rmSync(root, { force: true, recursive: true }));
    writeFileSync(
        path.join(root, "Cargo.toml"),
        `[workspace]\nresolver = "3"\nmembers = [${members.map(member => `"${member}"`).join(", ")}]\n`,
    );
    return root;
}

function crate(root: string, name: string): void {
    let directory = path.join(root, "crates", name);
    mkdirSync(path.join(directory, "src"), { recursive: true });
    writeFileSync(
        path.join(directory, "Cargo.toml"),
        `[package]\nname = "${name}"\nversion = "0.1.0"\nedition = "2024"\n`,
    );
    writeFileSync(path.join(directory, "src", "lib.rs"), "pub fn ready() -> bool { true }\n");
}

function run(root: string): { status: number | null; stderr: string } {
    let check = spawnSync(process.execPath, [checkCrates], { cwd: root, encoding: "utf8" });
    return { status: check.status, stderr: check.stderr };
}

test("rejects a crate the workspace does not claim", t => {
    let root = workspace(t, ["crates/registered"]);
    crate(root, "registered");
    crate(root, "unregistered");

    let check = run(root);

    assert.notEqual(check.status, 0);
    assert.match(check.stderr, /crates\/unregistered\/Cargo\.toml is not a workspace member/);
    assert.doesNotMatch(check.stderr, /crates\/registered\/Cargo\.toml/);
});

// `cargo metadata` writes its own warnings to the same stderr, so a passing run is
// checked by its exit status and the absence of a complaint, not by empty output.
test("accepts a workspace that claims every crate", t => {
    let root = workspace(t, ["crates/first", "crates/second"]);
    crate(root, "first");
    crate(root, "second");

    let check = run(root);

    assert.equal(check.status, 0, check.stderr);
    assert.doesNotMatch(check.stderr, /not a workspace member/);
});

test("ignores a crates entry that holds no manifest", t => {
    let root = workspace(t, ["crates/registered"]);
    crate(root, "registered");
    let fixtures = path.join(root, "crates", "fixtures");
    mkdirSync(fixtures, { recursive: true });
    writeFileSync(path.join(fixtures, "README.md"), "Not a crate.\n");

    let check = run(root);

    assert.equal(check.status, 0, check.stderr);
    assert.doesNotMatch(check.stderr, /not a workspace member/);
});
