import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

let buildCatalystFixture = fileURLToPath(new URL("./build-catalyst-fixture.ts", import.meta.url));
let repositoryRoot = fileURLToPath(new URL("..", import.meta.url));

test(
    "reports that the Catalyst fixture needs macOS",
    { skip: process.platform === "darwin" },
    t => {
        let root = mkdtempSync(path.join(tmpdir(), "recon-catalyst-fixture-"));
        t.after(() => rmSync(root, { force: true, recursive: true }));

        let build = spawnSync(process.execPath, [buildCatalystFixture], {
            cwd: root,
            encoding: "utf8",
        });

        assert.notEqual(build.status, 0);
        assert.match(build.stderr, /builds only on macOS/);
        assert.equal(existsSync(path.join(root, "target")), false);
    },
);

test(
    "builds a signed Mac Catalyst bundle, not an AppKit one",
    { skip: process.platform !== "darwin", timeout: 180_000 },
    () => {
        let build = spawnSync(process.execPath, [buildCatalystFixture], {
            cwd: repositoryRoot,
            encoding: "utf8",
        });
        assert.equal(build.status, 0, build.stderr);

        let bundle = path.join(repositoryRoot, "target/fixtures/ReconCatalystFixture.app");
        let executable = path.join(bundle, "Contents/MacOS/ReconCatalystFixture");
        assert.equal(existsSync(executable), true);
        assert.equal(existsSync(path.join(bundle, "Contents/Info.plist")), true);

        // The whole point of this fixture: the Mach-O platform says Mac Catalyst, so a Catalyst
        // row of the evidence matrix cannot be an AppKit run under another name.
        let load = spawnSync("otool", ["-l", executable], { encoding: "utf8" });
        assert.equal(load.status, 0, load.stderr);
        assert.match(load.stdout, /LC_BUILD_VERSION/);
        assert.match(load.stdout, /platform\s+(6|MACCATALYST)\b/);

        let signature = spawnSync("codesign", ["--verify", "--strict", bundle], {
            encoding: "utf8",
        });
        assert.equal(signature.status, 0, signature.stderr);

        // A session loads Recon's helper into this fixture, which only a cooperative target
        // allows.
        let entitlements = spawnSync("codesign", ["--display", "--entitlements", "-", bundle], {
            encoding: "utf8",
        });
        assert.match(
            entitlements.stdout + entitlements.stderr,
            /com\.apple\.security\.get-task-allow/,
        );
    },
);
