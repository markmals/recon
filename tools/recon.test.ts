import assert from "node:assert/strict";
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
import { before, test } from "node:test";

import { buildRecon, recon } from "./recon-cli.ts";
import { registerLaunchTests } from "./recon-launch-cases.ts";

before(buildRecon, { timeout: 600_000 });

test(
    "proposal.0001 inspects a selected bundle without launching it",
    { skip: process.platform !== "darwin" },
    t => {
        let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
        t.after(() => rmSync(root, { force: true, recursive: true }));
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

        let inspection = recon(["app", "inspect", bundle]);
        assert.equal(inspection.status, 0, inspection.stderr);
        assert.deepEqual(JSON.parse(inspection.stdout), {
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
        assert.equal(existsSync(marker), false);
    },
);

test(
    "proposal.0001 reports absent optional bundle metadata instead of inventing it",
    { skip: process.platform !== "darwin" },
    t => {
        let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
        t.after(() => rmSync(root, { force: true, recursive: true }));
        let bundle = path.join(root, "Bare App.app");
        mkdirSync(path.join(bundle, "Contents"), { recursive: true });
        writeFileSync(
            path.join(bundle, "Contents", "Info.plist"),
            '<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundlePackageType</key><string>APPL</string></dict></plist>',
        );

        let inspection = recon(["app", "inspect", bundle]);
        assert.equal(inspection.status, 0, inspection.stderr);
        assert.deepEqual(JSON.parse(inspection.stdout), {
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

test(
    "proposal.0001 rejects a directory that is not an application",
    { skip: process.platform !== "darwin" },
    t => {
        let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
        t.after(() => rmSync(root, { force: true, recursive: true }));
        let directory = path.join(root, "Not An App.app");
        mkdirSync(directory);
        let inspection = recon(["app", "inspect", directory]);
        assert.notEqual(inspection.status, 0);
        assert.equal(inspection.stdout, "");
        let { error } = JSON.parse(inspection.stderr);
        assert.equal(error.code, "NOT_AN_APPLICATION");
        assert.equal(error.details.reason, "unreadable-info-plist");
        assert.equal(error.details.path, realpathSync(directory));
    },
);

test(
    "proposal.0001 lists indexed bundles and resolves an identifier to its path",
    { skip: process.platform !== "darwin", timeout: 20_000 },
    () => {
        let listing = recon(["app", "list"]);
        assert.equal(listing.status, 0, listing.stderr);
        let inventory = JSON.parse(listing.stdout);
        assert.equal(inventory.discovery, "indexed-and-running");
        let calculator = inventory.applications.find(
            (app: { bundleId: string }) => app.bundleId === "com.apple.calculator",
        );
        assert.ok(Array.isArray(calculator?.runningPids));
        assert.deepEqual(calculator, {
            name: "Calculator",
            bundleId: "com.apple.calculator",
            path: "/System/Applications/Calculator.app",
            runningPids: calculator.runningPids,
        });
        let inspection = recon(["app", "inspect", "COM.APPLE.CALCULATOR"]);
        assert.equal(inspection.status, 0, inspection.stderr);
        assert.equal(JSON.parse(inspection.stdout).application.path, calculator.path);
    },
);

test(
    "proposal.0001 leaves foreground focus unchanged during list and inspect",
    { skip: process.platform !== "darwin", timeout: 20_000 },
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
            assert.equal(observation.status, 0, observation.stderr);
            return observation.stdout.trim();
        };
        let frontmost = foreground();
        assert.notEqual(frontmost, "-1");
        for (let args of [
            ["app", "list"],
            ["app", "inspect", "/System/Applications/Calculator.app"],
        ]) {
            let command = recon(args);
            assert.equal(command.status, 0, command.stderr);
            assert.equal(foreground(), frontmost);
        }
    },
);

test(
    "proposal.0001 uses the host display name when a bundle is renamed",
    { skip: process.platform !== "darwin" },
    t => {
        let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
        t.after(() => rmSync(root, { force: true, recursive: true }));
        let bundle = path.join(root, "Renamed App.app");
        mkdirSync(path.join(bundle, "Contents"), { recursive: true });
        writeFileSync(
            path.join(bundle, "Contents", "Info.plist"),
            '<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleDisplayName</key><string>Original Name</string></dict></plist>',
        );
        let inspection = recon(["app", "inspect", bundle]);
        assert.equal(inspection.status, 0, inspection.stderr);
        assert.equal(JSON.parse(inspection.stdout).application.name, "Renamed App");
    },
);

test(
    "proposal.0001 reports unsupported hosts instead of a successful empty list",
    { skip: process.platform === "darwin" },
    () => {
        let listing = recon(["app", "list"]);
        assert.notEqual(listing.status, 0);
        assert.equal(listing.stdout, "");
        let { error } = JSON.parse(listing.stderr);
        assert.equal(error.code, "UNSUPPORTED_HOST");
        assert.equal(
            error.details.host,
            process.platform === "win32" ? "windows" : process.platform,
        );
        assert.deepEqual(error.details.supportedHosts, ["macos"]);
    },
);

registerLaunchTests();
