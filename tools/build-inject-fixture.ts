import { execFileSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

const ENTITLEMENTS = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.get-task-allow</key>
    <true/>
</dict>
</plist>
`;

function buildInjectFixture(): void {
    if (process.platform !== "darwin") {
        console.error(
            "The injection fixture builds only on macOS with the Xcode command-line tools.",
        );
        process.exitCode = 1;
        return;
    }

    let source = path.resolve("crates/recon/tests/fixtures/inject-fixture/main.c");
    let outDir = path.resolve("target/fixtures");
    let binary = path.join(outDir, "inject-fixture");
    let entitlements = path.join(outDir, "inject-fixture.entitlements.plist");

    mkdirSync(outDir, { recursive: true });
    rmSync(binary, { force: true });

    // Build both Apple-silicon slices so the loader's thread-state handling can be exercised
    // against arm64 and arm64e targets from one fixture.
    execFileSync(
        "clang",
        [
            "-arch",
            "arm64",
            "-arch",
            "arm64e",
            "-framework",
            "CoreFoundation",
            "-O2",
            source,
            "-o",
            binary,
        ],
        { stdio: "inherit" },
    );

    writeFileSync(entitlements, ENTITLEMENTS);

    execFileSync(
        "clang",
        [
            "-dynamiclib",
            "-arch",
            "arm64",
            "-arch",
            "arm64e",
            "-O2",
            path.resolve("crates/recon/tests/fixtures/inject-fixture/helper.c"),
            "-o",
            path.join(outDir, "inject-helper.dylib"),
        ],
        { stdio: "inherit" },
    );

    // Ad-hoc is enough: cooperation comes from get-task-allow, not from a stable identity.
    execFileSync("codesign", ["--force", "--sign", "-", "--entitlements", entitlements, binary], {
        stdio: "inherit",
    });

    console.log(`Built ${path.relative(process.cwd(), binary)} (arm64, arm64e; get-task-allow)`);
}

buildInjectFixture();
