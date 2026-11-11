import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";

// The application that holds the user's place while the interaction evidence runs: it stays
// frontmost, keeps the keyboard focus, and covers the fixtures. It needs no entitlement — no
// session is ever opened on it — so an ad-hoc signature is all macOS asks for to launch it.
function buildReceiver(): void {
    if (process.platform !== "darwin") {
        console.error(
            "The Recon foreground receiver is an AppKit application; it builds only on macOS with the Xcode command-line tools.",
        );
        process.exitCode = 1;
        return;
    }

    let source = path.resolve("crates/recon/tests/fixtures/ReconForegroundReceiver");
    let bundle = path.resolve("target/fixtures/ReconForegroundReceiver.app");
    let executables = path.join(bundle, "Contents", "MacOS");

    rmSync(bundle, { force: true, recursive: true });
    mkdirSync(executables, { recursive: true });
    execFileSync(
        "xcrun",
        [
            "--sdk",
            "macosx",
            "swiftc",
            "-O",
            path.join(source, "main.swift"),
            "-o",
            path.join(executables, "ReconForegroundReceiver"),
        ],
        { stdio: "inherit" },
    );
    copyFileSync(path.join(source, "Info.plist"), path.join(bundle, "Contents", "Info.plist"));
    execFileSync("codesign", ["--force", "--sign", "-", bundle], { stdio: "inherit" });

    console.log(`Built ${path.relative(process.cwd(), bundle)}`);
}

buildReceiver();
