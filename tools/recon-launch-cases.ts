import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, mkdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { expect, onTestFinished, test } from "vite-plus/test";

import { recon } from "./recon-cli.ts";

export function registerLaunchTests(): void {
    test.skipIf(process.platform !== "darwin")(
        "proposal.0001 reports an unlaunchable selected bundle without claiming success",
        () => {
            let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
            onTestFinished(() => rmSync(root, { force: true, recursive: true }));
            let bundle = path.join(root, "Broken App.app");
            mkdirSync(path.join(bundle, "Contents"), { recursive: true });
            writeFileSync(
                path.join(bundle, "Contents", "Info.plist"),
                '<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundlePackageType</key><string>APPL</string><key>CFBundleExecutable</key><string>missing</string></dict></plist>',
            );
            let launch = recon(["app", "launch", bundle], { timeout: 20_000 });
            expect(launch.status).not.toBe(0);
            expect(launch.stdout).toBe("");
            expect(JSON.parse(launch.stderr).error).toMatchObject({
                code: "LAUNCH_FAILED",
                retryable: false,
                details: { application: realpathSync(bundle), reason: "executable-missing" },
            });
        },
    );

    test.skipIf(process.platform !== "darwin")(
        "proposal.0001 launches one selected bundle and reports its actual process",
        () => {
            let root = mkdtempSync(path.join(tmpdir(), "recon-app-"));
            let bundle = path.join(root, "Background App.app");
            let executable = path.join(bundle, "Contents", "MacOS", "background");
            let pid: number | undefined;
            let copyPid: number | undefined;
            onTestFinished(() => {
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
            expect(compiled.status, compiled.stderr).toBe(0);

            let first = recon(["app", "launch", bundle], { timeout: 20_000 });
            expect(first.status, first.stderr).toBe(0);
            let opened = JSON.parse(first.stdout);
            pid = opened.pid;
            expect(opened.path).toBe(realpathSync(bundle));
            expect(pid).toBeGreaterThan(0);
            let second = recon(["app", "launch", bundle], { timeout: 20_000 });
            expect(second.status, second.stderr).toBe(0);
            expect(JSON.parse(second.stdout)).toEqual(opened);
            let inspection = recon(["app", "inspect", bundle]);
            expect(inspection.status, inspection.stderr).toBe(0);
            expect(JSON.parse(inspection.stdout).application.runningPids).toContain(pid);
            let listing = recon(["app", "list"]);
            expect(listing.status, listing.stderr).toBe(0);
            let listed = JSON.parse(listing.stdout).applications.find(
                (app: { path: string }) => app.path === realpathSync(bundle),
            );
            expect(listed).toEqual({
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
                expect(selected.path).toBe(realpathSync(copy));
                expect(copyPid).not.toBe(pid);
            } else {
                expect(another.stdout).toBe("");
                expect(JSON.parse(another.stderr).error).toMatchObject({
                    code: "LAUNCH_FAILED",
                    details: { application: realpathSync(copy) },
                });
            }
            let original = recon(["app", "inspect", bundle]);
            expect(original.status, original.stderr).toBe(0);
            expect(JSON.parse(original.stdout).application.runningPids).toContain(pid);
        },
        60_000,
    );
}
