import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

// Mac Catalyst is a deployment target of the macOS SDK: the iOS frameworks and Swift modules
// live under `System/iOSSupport`, and the `-macabi` triple is what makes the built executable
// carry `LC_BUILD_VERSION` platform `MACCATALYST` rather than `MACOS`.
const CATALYST_TARGET = "arm64-apple-ios18.0-macabi";

// The Mach-O platform of Mac Catalyst, as `otool` names or numbers it. An AppKit build
// labelled Catalyst would prove nothing about Catalyst, so the build refuses to produce one.
const MACCATALYST_PLATFORM = ["6", "MACCATALYST"];

function buildCatalystFixture(): void {
    if (process.platform !== "darwin") {
        console.error(
            "The Recon Catalyst fixture is a Mac Catalyst application; it builds only on macOS with Xcode's macOS SDK.",
        );
        process.exitCode = 1;
        return;
    }

    let source = path.resolve("crates/recon/tests/fixtures/ReconCatalystFixture");
    let bundle = path.resolve("target/fixtures/ReconCatalystFixture.app");
    let executables = path.join(bundle, "Contents", "MacOS");
    let sdk = execFileSync("xcrun", ["--sdk", "macosx", "--show-sdk-path"], {
        encoding: "utf8",
    }).trim();
    let iosSupport = path.join(sdk, "System/iOSSupport");

    rmSync(bundle, { force: true, recursive: true });
    mkdirSync(executables, { recursive: true });
    let executable = path.join(executables, "ReconCatalystFixture");
    execFileSync(
        "xcrun",
        [
            "swiftc",
            "-O",
            "-target",
            CATALYST_TARGET,
            "-sdk",
            sdk,
            "-F",
            path.join(iosSupport, "System/Library/Frameworks"),
            "-I",
            path.join(iosSupport, "usr/lib/swift"),
            "-L",
            path.join(iosSupport, "usr/lib/swift"),
            path.join(source, "main.swift"),
            "-o",
            executable,
        ],
        { stdio: "inherit" },
    );
    copyFileSync(path.join(source, "Info.plist"), path.join(bundle, "Contents", "Info.plist"));

    let platform = buildPlatform(executable);
    if (!MACCATALYST_PLATFORM.includes(platform)) {
        console.error(
            `${executable} reports LC_BUILD_VERSION platform ${platform || "none"}, not Mac Catalyst.`,
        );
        process.exitCode = 1;
        return;
    }

    let entitlements = path.resolve("target/fixtures", "ReconCatalystFixture.entitlements.plist");
    writeFileSync(
        entitlements,
        `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>com.apple.security.get-task-allow</key><true/></dict></plist>
`,
    );
    // The fixture explicitly cooperates with Recon's standard-profile loader, which a Catalyst
    // key chord needs: it runs through the helper in an open session.
    execFileSync("codesign", ["--force", "--sign", "-", "--entitlements", entitlements, bundle], {
        stdio: "inherit",
    });

    console.log(
        `Built ${path.relative(process.cwd(), bundle)} (${CATALYST_TARGET}; LC_BUILD_VERSION platform ${platform})`,
    );
}

// The `LC_BUILD_VERSION` platform of a built executable, as `otool` reports it: a number on
// some toolchains and a name on others.
function buildPlatform(executable: string): string {
    let load = execFileSync("otool", ["-l", executable], { encoding: "utf8" });
    let lines = load.split("\n");
    for (let index = 0; index < lines.length; index += 1) {
        if (!lines[index].includes("LC_BUILD_VERSION")) continue;
        for (let field = index; field < Math.min(index + 6, lines.length); field += 1) {
            let platform = /^\s*platform\s+(\S+)\s*$/.exec(lines[field]);
            if (platform) return platform[1];
        }
    }
    return "";
}

buildCatalystFixture();
