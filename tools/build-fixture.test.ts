import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, onTestFinished, test } from "vite-plus/test";

let buildFixture = fileURLToPath(new URL("./build-fixture.ts", import.meta.url));
let repositoryRoot = fileURLToPath(new URL("..", import.meta.url));

test.skipIf(process.platform === "darwin")("reports that the fixture needs macOS", () => {
    let root = mkdtempSync(path.join(tmpdir(), "recon-fixture-"));
    onTestFinished(() => rmSync(root, { force: true, recursive: true }));

    let build = spawnSync(process.execPath, [buildFixture], { cwd: root, encoding: "utf8" });

    expect(build.status).not.toBe(0);
    expect(build.stderr).toContain("builds only on macOS");
    expect(existsSync(path.join(root, "target"))).toBe(false);
});

test.skipIf(process.platform !== "darwin")(
    "builds a signed fixture bundle macOS can launch",
    () => {
        let build = spawnSync(process.execPath, [buildFixture], {
            cwd: repositoryRoot,
            encoding: "utf8",
        });
        expect(build.status, build.stderr).toBe(0);

        let bundle = path.join(repositoryRoot, "target/fixtures/ReconFixture.app");
        expect(existsSync(path.join(bundle, "Contents/MacOS/ReconFixture"))).toBe(true);
        expect(existsSync(path.join(bundle, "Contents/Info.plist"))).toBe(true);

        let signature = spawnSync("codesign", ["--verify", "--strict", bundle], {
            encoding: "utf8",
        });
        expect(signature.status, signature.stderr).toBe(0);

        let identifier = spawnSync(
            "defaults",
            ["read", path.join(bundle, "Contents/Info"), "CFBundleIdentifier"],
            { encoding: "utf8" },
        );
        expect(identifier.stdout.trim()).toBe("me.malstrom.recon.fixture");
    },
    120_000,
);
