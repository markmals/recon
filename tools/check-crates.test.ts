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

interface CrateOptions {
    /** Crate names this one depends on for every target. */
    dependencies?: string[];
    /** Crate names this one depends on only under `cfg(target_os = "macos")`. */
    macosDependencies?: string[];
    /** External dependencies declared for every target, as `name = declaration`. */
    external?: Record<string, string>;
}

function crate(root: string, name: string, options: CrateOptions = {}): void {
    let directory = path.join(root, "crates", name);
    mkdirSync(path.join(directory, "src"), { recursive: true });
    let local = (dependency: string) => [dependency, `{ path = "../${dependency}" }`] as const;
    writeFileSync(
        path.join(directory, "Cargo.toml"),
        [
            `[package]\nname = "${name}"\nversion = "0.1.0"\nedition = "2024"\n`,
            table("[dependencies]", [
                ...(options.dependencies ?? []).map(local),
                ...Object.entries(options.external ?? {}),
            ]),
            table(
                `[target.'cfg(target_os = "macos")'.dependencies]`,
                (options.macosDependencies ?? []).map(local),
            ),
        ].join(""),
    );
    writeFileSync(path.join(directory, "src", "lib.rs"), "pub fn ready() -> bool { true }\n");
}

/** One Cargo dependency table, or nothing when it would be empty. */
function table(heading: string, entries: readonly (readonly [string, string])[]): string {
    if (entries.length === 0) return "";
    return `\n${heading}\n${entries.map(([name, value]) => `${name} = ${value}`).join("\n")}\n`;
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

test("rejects a core that depends on a capability crate", t => {
    let root = workspace(t, ["crates/recon-core", "crates/recon-windows-macos"]);
    crate(root, "recon-core", { dependencies: ["recon-windows-macos"] });
    crate(root, "recon-windows-macos");

    let check = run(root);

    assert.notEqual(check.status, 0);
    assert.match(check.stderr, /recon-core depends on recon-windows-macos/);
});

test("rejects a crate that depends on the executable", t => {
    let root = workspace(t, ["crates/recon", "crates/recon-windows-macos"]);
    crate(root, "recon");
    crate(root, "recon-windows-macos", { dependencies: ["recon"] });

    let check = run(root);

    assert.notEqual(check.status, 0);
    assert.match(check.stderr, /recon-windows-macos depends on recon;/);
});

test("rejects a portable crate that depends on a platform crate", t => {
    let root = workspace(t, ["crates/recon-support", "crates/recon-windows-macos"]);
    crate(root, "recon-support", { dependencies: ["recon-windows-macos"] });
    crate(root, "recon-windows-macos");

    let check = run(root);

    assert.notEqual(check.status, 0);
    assert.match(check.stderr, /recon-support depends on recon-windows-macos/);
});

test("rejects a platform crate whose Apple dependencies are not target-gated", t => {
    let root = workspace(t, ["crates/recon-windows-macos"]);
    crate(root, "recon-windows-macos", { external: { "objc2-foundation": `"0.3"` } });

    let check = run(root);

    assert.notEqual(check.status, 0);
    assert.match(check.stderr, /recon-windows-macos depends on objc2-foundation for every target/);
});

test("accepts the proposed dependency direction", t => {
    let root = workspace(t, [
        "crates/recon",
        "crates/recon-core",
        "crates/recon-windows-macos",
        "crates/recon-guard-macos",
    ]);
    crate(root, "recon", {
        dependencies: ["recon-core", "recon-windows-macos", "recon-guard-macos"],
    });
    crate(root, "recon-core");
    crate(root, "recon-windows-macos", {
        dependencies: ["recon-core"],
        macosDependencies: [],
    });
    crate(root, "recon-guard-macos", { dependencies: ["recon-core", "recon-windows-macos"] });

    let check = run(root);

    assert.equal(check.status, 0, check.stderr);
});
