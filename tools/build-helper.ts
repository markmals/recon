import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

function buildHelper(): void {
    if (process.platform !== "darwin") {
        console.error(
            "The Recon helper is a native dylib; it builds only on macOS with the Xcode command-line tools.",
        );
        process.exitCode = 1;
        return;
    }

    let sources = [
        "helper/recon_helper.c",
        "helper/ReconCore.m",
        "helper/ReconFields.m",
        "helper/ReconFacets.m",
        "helper/ReconInspect.m",
        "helper/ReconServer.m",
    ].map(source => path.resolve(source));
    let outDir = path.resolve("target");
    let dylib = path.join(outDir, "recon-helper.dylib");
    let buildIdPath = path.join(outDir, "recon-helper.build-id");
    let headers = readdirSync("helper")
        .filter(file => file.endsWith(".h"))
        .sort()
        .map(file => path.resolve("helper", file));
    let hash = createHash("sha256");
    for (let source of [...sources, ...headers]) hash.update(readFileSync(source));
    hash.update("arm64+arm64e|arc|ReconProtocolVersion=1");
    let buildId = hash.digest("hex");

    mkdirSync(outDir, { recursive: true });
    rmSync(dylib, { force: true });
    rmSync(buildIdPath, { force: true });

    // Build both Apple-silicon slices: a process can only load a dylib of its own architecture, so
    // the helper must match arm64 and arm64e targets alike.
    execFileSync(
        "clang",
        [
            "-dynamiclib",
            "-arch",
            "arm64",
            "-arch",
            "arm64e",
            "-fobjc-arc",
            "-O2",
            `-DRECON_HELPER_BUILD="${buildId}"`,
            ...sources,
            "-framework",
            "Foundation",
            "-framework",
            "AppKit",
            "-framework",
            "QuartzCore",
            "-framework",
            "CoreGraphics",
            "-framework",
            "Security",
            "-lbsm",
            "-lproc",
            "-o",
            dylib,
        ],
        { stdio: "inherit" },
    );

    // Ad-hoc is enough to load into a cooperative target; the research profile re-signs the whole
    // inspector, helper included, with the entitlements the platform-binary path needs.
    execFileSync("codesign", ["--force", "--sign", "-", dylib], { stdio: "inherit" });
    writeFileSync(buildIdPath, `${buildId}\n`, { mode: 0o600 });

    console.log(`Built ${path.relative(process.cwd(), dylib)} (arm64, arm64e; ${buildId})`);
}

buildHelper();
