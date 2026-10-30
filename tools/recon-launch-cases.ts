import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, mkdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";

import { recon } from "./recon-cli.ts";

export function registerLaunchTests(): void {
    test(
        "proposal.0001 reports an unlaunchable selected bundle without claiming success",
        { skip: process.platform !== "darwin" },
        t => {
            let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
            t.after(() => rmSync(root, { force: true, recursive: true }));
            let bundle = path.join(root, "Broken App.app");
            mkdirSync(path.join(bundle, "Contents"), { recursive: true });
            writeFileSync(
                path.join(bundle, "Contents", "Info.plist"),
                '<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleExecutable</key><string>missing</string></dict></plist>',
            );
            let launch = recon(["app", "launch", bundle], { timeout: 20_000 });
            assert.notEqual(launch.status, 0);
            assert.equal(launch.stdout, "");
            let { error } = JSON.parse(launch.stderr);
            assert.equal(error.code, "LAUNCH_FAILED");
            assert.equal(error.retryable, false);
            assert.equal(error.details.application, realpathSync(bundle));
            assert.equal(error.details.reason, "executable-missing");
        },
    );

    test(
        "proposal.0001 launches one selected bundle and reports its actual process",
        { skip: process.platform !== "darwin", timeout: 60_000 },
        t => {
            let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
            let bundle = path.join(root, "Background App.app");
            let executable = path.join(bundle, "Contents", "MacOS", "background");
            let pid: number | undefined;
            let copyPid: number | undefined;
            t.after(() => {
                if (pid) {
                    try {
                        process.kill(pid);
                    } catch (error) {
                        if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
                    }
                }
                if (copyPid && copyPid !== pid) {
                    try {
                        process.kill(copyPid);
                    } catch (error) {
                        if ((error as NodeJS.ErrnoException).code !== "ESRCH") throw error;
                    }
                }
                rmSync(root, { force: true, recursive: true });
            });
            mkdirSync(path.dirname(executable), { recursive: true });
            writeFileSync(
                path.join(bundle, "Contents", "Info.plist"),
                '<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleIdentifier</key><string>example.recon.background</string><key>CFBundleExecutable</key><string>background</string><key>LSUIElement</key><true/></dict></plist>',
            );
            let compiled = spawnSync(
                "cc",
                ["-x", "objective-c", "-framework", "AppKit", "-o", executable, "-"],
                {
                    input: "#import <AppKit/AppKit.h>\nint main(void) { @autoreleasepool { [NSApplication sharedApplication]; [NSApp run]; } }\n",
                    encoding: "utf8",
                },
            );
            assert.equal(compiled.status, 0, compiled.stderr);

            let first = recon(["app", "launch", bundle], { timeout: 20_000 });
            assert.equal(first.status, 0, first.stderr);
            let opened = JSON.parse(first.stdout);
            pid = opened.pid;
            assert.equal(opened.path, realpathSync(bundle));
            assert.ok(pid !== undefined && pid > 0, `expected a launched pid, got ${pid}`);
            let second = recon(["app", "launch", bundle], { timeout: 20_000 });
            assert.equal(second.status, 0, second.stderr);
            assert.deepEqual(JSON.parse(second.stdout), opened);
            let inspection = recon(["app", "inspect", bundle]);
            assert.equal(inspection.status, 0, inspection.stderr);
            assert.ok(JSON.parse(inspection.stdout).application.runningPids.includes(pid));
            let listing = recon(["app", "list"]);
            assert.equal(listing.status, 0, listing.stderr);
            let listed = JSON.parse(listing.stdout).applications.find(
                (app: { path: string }) => app.path === realpathSync(bundle),
            );
            assert.deepEqual(listed, {
                name: "Background App",
                bundleId: "example.recon.background",
                path: realpathSync(bundle),
                runningPids: [pid],
            });
            let copy = path.join(root, "Other Copy.app");
            cpSync(bundle, copy, { recursive: true });
            let another = recon(["app", "launch", copy], { timeout: 20_000 });
            if (another.status === 0) {
                let selected = JSON.parse(another.stdout);
                copyPid = selected.pid;
                assert.equal(selected.path, realpathSync(copy));
                assert.notEqual(copyPid, pid);
            } else {
                assert.equal(another.stdout, "");
                let { error } = JSON.parse(another.stderr);
                assert.equal(error.code, "LAUNCH_FAILED");
                assert.equal(error.details.application, realpathSync(copy));
            }
            let original = recon(["app", "inspect", bundle]);
            assert.equal(original.status, 0, original.stderr);
            assert.ok(JSON.parse(original.stdout).application.runningPids.includes(pid));
        },
    );
}
