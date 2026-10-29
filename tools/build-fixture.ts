import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
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
    let entitlements = path.join("target/fixtures", "ReconFixture.entitlements.plist");
    writeFileSync(
        entitlements,
        `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>com.apple.security.get-task-allow</key><true/></dict></plist>
`,
    );
    // The fixture explicitly cooperates with Recon's standard-profile loader.
    execFileSync("codesign", ["--force", "--sign", "-", "--entitlements", entitlements, bundle], {
        stdio: "inherit",
    });

    console.log(`Built ${path.relative(process.cwd(), bundle)}`);
}

buildFixture();
