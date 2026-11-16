import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

let buildReceiver = fileURLToPath(new URL("./build-receiver.ts", import.meta.url));
let repositoryRoot = fileURLToPath(new URL("..", import.meta.url));

test("reports that the receiver needs macOS", { skip: process.platform === "darwin" }, t => {
    let root = mkdtempSync(path.join(tmpdir(), "recon-receiver-"));
    t.after(() => rmSync(root, { force: true, recursive: true }));

    let build = spawnSync(process.execPath, [buildReceiver], { cwd: root, encoding: "utf8" });

    assert.notEqual(build.status, 0);
    assert.match(build.stderr, /builds only on macOS/);
    assert.equal(existsSync(path.join(root, "target")), false);
});

test(
    "builds a signed receiver bundle macOS can launch",
    { skip: process.platform !== "darwin", timeout: 120_000 },
    () => {
        let build = spawnSync(process.execPath, [buildReceiver], {
            cwd: repositoryRoot,
            encoding: "utf8",
        });
        assert.equal(build.status, 0, build.stderr);

        let bundle = path.join(repositoryRoot, "target/fixtures/ReconForegroundReceiver.app");
        assert.equal(existsSync(path.join(bundle, "Contents/MacOS/ReconForegroundReceiver")), true);
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
        assert.equal(identifier.stdout.trim(), "me.malstrom.recon.receiver");
    },
);
