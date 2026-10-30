import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

let buildFixture = fileURLToPath(new URL("./build-fixture.ts", import.meta.url));
let repositoryRoot = fileURLToPath(new URL("..", import.meta.url));

test("reports that the fixture needs macOS", { skip: process.platform === "darwin" }, t => {
    let root = mkdtempSync(path.join(tmpdir(), "recon-fixture-"));
    t.after(() => rmSync(root, { force: true, recursive: true }));

    let build = spawnSync(process.execPath, [buildFixture], { cwd: root, encoding: "utf8" });

    assert.notEqual(build.status, 0);
    assert.match(build.stderr, /builds only on macOS/);
    assert.equal(existsSync(path.join(root, "target")), false);
});

test(
    "builds a signed fixture bundle macOS can launch",
    { skip: process.platform !== "darwin", timeout: 120_000 },
    () => {
        let build = spawnSync(process.execPath, [buildFixture], {
            cwd: repositoryRoot,
            encoding: "utf8",
        });
        assert.equal(build.status, 0, build.stderr);

        let bundle = path.join(repositoryRoot, "target/fixtures/ReconFixture.app");
        assert.equal(existsSync(path.join(bundle, "Contents/MacOS/ReconFixture")), true);
        assert.equal(existsSync(path.join(bundle, "Contents/Info.plist")), true);

        let signature = spawnSync("codesign", ["--verify", "--strict", bundle], {
            encoding: "utf8",
        });
        assert.equal(signature.status, 0, signature.stderr);

        let identifier = spawnSync(
            "defaults",
            ["read", path.join(bundle, "Contents/Info"), "CFBundleIdentifier"],
            { encoding: "utf8" },
        );
        assert.equal(identifier.stdout.trim(), "me.malstrom.recon.fixture");
    },
);
