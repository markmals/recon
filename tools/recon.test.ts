import { spawnSync } from "node:child_process";
import {
    chmodSync,
    existsSync,
    mkdtempSync,
    mkdirSync,
    realpathSync,
    rmSync,
    writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, onTestFinished, test } from "vite-plus/test";

import { registerLaunchTests } from "./recon-launch-cases.ts";

let manifest = fileURLToPath(new URL("../crates/recon/Cargo.toml", import.meta.url));

test.skipIf(process.platform !== "darwin")(
    "proposal.0001 inspects a selected bundle without launching it",
    () => {
        let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
        onTestFinished(() => rmSync(root, { force: true, recursive: true }));
        let bundle = path.join(root, "Recon Fixture.app");
        let macOS = path.join(bundle, "Contents", "MacOS");
        let marker = path.join(root, "launched");
        mkdirSync(macOS, { recursive: true });
        writeFileSync(
            path.join(bundle, "Contents", "Info.plist"),
            `<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleIdentifier</key><string>example.recon.fixture</string>
<key>CFBundleDisplayName</key><string>Recon Fixture</string>
<key>CFBundleExecutable</key><string>fixture</string>
<key>CFBundleShortVersionString</key><string>1.2</string>
<key>CFBundleVersion</key><string>3</string>
</dict></plist>`,
        );
        writeFileSync(path.join(macOS, "fixture"), `#!/bin/sh\ntouch '${marker}'\n`);
        chmodSync(path.join(macOS, "fixture"), 0o755);

        let inspection = spawnSync(
            "cargo",
            ["run", "--quiet", "--manifest-path", manifest, "--", "app", "inspect", bundle],
            {
                encoding: "utf8",
            },
        );
        expect(inspection.status, inspection.stderr).toBe(0);
        expect(JSON.parse(inspection.stdout)).toEqual({
            application: {
                name: "Recon Fixture",
                bundleId: "example.recon.fixture",
                path: realpathSync(bundle),
                version: "1.2",
                build: "3",
                executablePath: path.join(realpathSync(bundle), "Contents", "MacOS", "fixture"),
                runningPids: [],
            },
        });
        expect(existsSync(marker)).toBe(false);
    },
    60_000,
);

test.skipIf(process.platform !== "darwin")(
    "proposal.0001 reports absent optional bundle metadata instead of inventing it",
    () => {
        let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
        onTestFinished(() => rmSync(root, { force: true, recursive: true }));
        let bundle = path.join(root, "Bare App.app");
        mkdirSync(path.join(bundle, "Contents"), { recursive: true });
        writeFileSync(
            path.join(bundle, "Contents", "Info.plist"),
            '<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundlePackageType</key><string>APPL</string></dict></plist>',
        );

        let inspection = spawnSync(
            "cargo",
            ["run", "--quiet", "--manifest-path", manifest, "--", "app", "inspect", bundle],
            { encoding: "utf8" },
        );
        expect(inspection.status, inspection.stderr).toBe(0);
        expect(JSON.parse(inspection.stdout)).toEqual({
            application: {
                name: "Bare App",
                bundleId: null,
                path: realpathSync(bundle),
                version: null,
                build: null,
                executablePath: null,
                runningPids: [],
            },
        });
    },
);

test.skipIf(process.platform !== "darwin")(
    "proposal.0001 rejects a directory that is not an application",
    () => {
        let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
        onTestFinished(() => rmSync(root, { force: true, recursive: true }));
        let directory = path.join(root, "Not An App.app");
        mkdirSync(directory);
        let inspection = spawnSync(
            "cargo",
            ["run", "--quiet", "--manifest-path", manifest, "--", "app", "inspect", directory],
            { encoding: "utf8" },
        );
        expect(inspection.status).not.toBe(0);
        expect(inspection.stdout).toBe("");
        expect(inspection.stderr).toContain("not an application");
    },
);

test.skipIf(process.platform !== "darwin")(
    "proposal.0001 lists indexed bundles and resolves an identifier to its path",
    () => {
        let listing = spawnSync(
            "cargo",
            ["run", "--quiet", "--manifest-path", manifest, "--", "app", "list"],
            {
                encoding: "utf8",
            },
        );
        expect(listing.status, listing.stderr).toBe(0);
        let inventory = JSON.parse(listing.stdout);
        expect(inventory.discovery).toBe("indexed-and-running");
        let calculator = inventory.applications.find(
            (app: { bundleId: string }) => app.bundleId === "com.apple.calculator",
        );
        expect(calculator).toEqual({
            name: "Calculator",
            bundleId: "com.apple.calculator",
            path: "/System/Applications/Calculator.app",
            runningPids: expect.any(Array),
        });
        let inspection = spawnSync(
            "cargo",
            [
                "run",
                "--quiet",
                "--manifest-path",
                manifest,
                "--",
                "app",
                "inspect",
                "COM.APPLE.CALCULATOR",
            ],
            { encoding: "utf8" },
        );
        expect(inspection.status, inspection.stderr).toBe(0);
        expect(JSON.parse(inspection.stdout).application.path).toBe(calculator.path);
    },
);

test.skipIf(process.platform !== "darwin")(
    "proposal.0001 leaves foreground focus unchanged during list and inspect",
    () => {
        let foreground = () => {
            let observation = spawnSync(
                "swift",
                [
                    "-e",
                    "import AppKit; print(NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1)",
                ],
                { encoding: "utf8" },
            );
            expect(observation.status, observation.stderr).toBe(0);
            return observation.stdout.trim();
        };
        let before = foreground();
        expect(before).not.toBe("-1");
        for (let args of [
            ["app", "list"],
            ["app", "inspect", "/System/Applications/Calculator.app"],
        ]) {
            let command = spawnSync(
                "cargo",
                ["run", "--quiet", "--manifest-path", manifest, "--", ...args],
                { encoding: "utf8" },
            );
            expect(command.status, command.stderr).toBe(0);
            expect(foreground()).toBe(before);
        }
    },
    20_000,
);

test.skipIf(process.platform !== "darwin")(
    "proposal.0001 uses the host display name when a bundle is renamed",
    () => {
        let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
        onTestFinished(() => rmSync(root, { force: true, recursive: true }));
        let bundle = path.join(root, "Renamed App.app");
        mkdirSync(path.join(bundle, "Contents"), { recursive: true });
        writeFileSync(
            path.join(bundle, "Contents", "Info.plist"),
            '<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleDisplayName</key><string>Original Name</string></dict></plist>',
        );
        let inspection = spawnSync(
            "cargo",
            ["run", "--quiet", "--manifest-path", manifest, "--", "app", "inspect", bundle],
            { encoding: "utf8" },
        );
        expect(inspection.status, inspection.stderr).toBe(0);
        expect(JSON.parse(inspection.stdout).application.name).toBe("Renamed App");
    },
);

test.skipIf(process.platform === "darwin")(
    "proposal.0001 reports unsupported hosts instead of a successful empty list",
    () => {
        let listing = spawnSync(
            "cargo",
            ["run", "--quiet", "--manifest-path", manifest, "--", "app", "list"],
            {
                encoding: "utf8",
            },
        );
        expect(listing.status).not.toBe(0);
        expect(listing.stdout).toBe("");
        expect(listing.stderr).toContain("supported only on macOS");
    },
    60_000,
);

registerLaunchTests();
