import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync } from "node:fs";
import path from "node:path";

function buildFixture(): void {
    if (process.platform !== "darwin") {
        console.error(
            "The Recon fixture is an AppKit application; it builds only on macOS with the Xcode command-line tools.",
        );
        process.exitCode = 1;
        return;
    }

    let source = path.resolve("crates/recon/tests/fixtures/ReconFixture");
    let bundle = path.resolve("target/fixtures/ReconFixture.app");
    let executables = path.join(bundle, "Contents", "MacOS");

    rmSync(bundle, { force: true, recursive: true });
    mkdirSync(executables, { recursive: true });
    execFileSync(
        "swiftc",
        ["-O", path.join(source, "main.swift"), "-o", path.join(executables, "ReconFixture")],
        { stdio: "inherit" },
    );
    copyFileSync(path.join(source, "Info.plist"), path.join(bundle, "Contents", "Info.plist"));
    // Ad-hoc, because the fixture is observed rather than an observer: it needs no privacy grant.
    execFileSync("codesign", ["--force", "--sign", "-", bundle], { stdio: "inherit" });

    console.log(`Built ${path.relative(process.cwd(), bundle)}`);
}

buildFixture();
