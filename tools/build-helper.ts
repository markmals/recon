// Builds the universal target helper and the build identity the CLI must match.
//
// The identity hashes repository-relative input paths and contents, the resolved package pins,
// the compiler, linker and SDK identities, and the build configuration — never a timestamp or a
// checkout path. The same inputs therefore produce the same signed bytes from any checkout,
// which is what makes a helper and a `recon` build verifiably matched.
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";

import { verifiedCompiler } from "./swift-toolchain.ts";

/// Everything the helper's behaviour depends on, beyond the sources themselves.
const CONFIGURATION = "arm64+arm64e|release|macOS27.0|ld -S|ReconProtocolVersion=1";

const MANIFESTS = [
    "sdks/swift/Package.swift",
    "sdks/swift/Package.resolved",
    "extensions/appkit/Package.swift",
    "extensions/appkit/Package.resolved",
    "helper/Package.swift",
    "helper/Package.resolved",
];

const SOURCE_TREES = ["sdks/swift/Sources", "extensions/appkit/Sources", "helper/Sources"];

/** Code-unit order, not locale order, so identity does not depend on the builder's locale. */
function byCodeUnit(left: string, right: string): number {
    return left < right ? -1 : left > right ? 1 : 0;
}

function sourceFiles(directory: string): string[] {
    return readdirSync(directory, { withFileTypes: true })
        .flatMap(entry => {
            let full = path.join(directory, entry.name);
            return entry.isDirectory() ? sourceFiles(full) : [full];
        })
        .sort(byCodeUnit);
}

function buildIdentity(compiler: string): string {
    let inputs = [...MANIFESTS, ...SOURCE_TREES.flatMap(sourceFiles)].sort(byCodeUnit);
    let hash = createHash("sha256");
    for (let input of inputs) {
        hash.update(`${input}\0`);
        hash.update(readFileSync(path.resolve(input)));
        hash.update("\0");
    }
    // The compiler string alone is not enough: Xcode and the Command Line Tools ship the same
    // swiftlang build with different linkers, and their helpers differ byte for byte.
    hash.update(compiler);
    hash.update(spawnSync("xcrun", ["ld", "-v"], { encoding: "utf8" }).stderr); // `ld -v` reports on stderr
    hash.update(execFileSync("xcrun", ["--show-sdk-build-version"], { encoding: "utf8" }));
    hash.update(CONFIGURATION);
    return hash.digest("hex");
}

function buildHelper(): void {
    if (process.platform !== "darwin") {
        console.error(
            "The Recon helper is a native dylib; it builds only on macOS with Apple's Swift " +
                "toolchain.",
        );
        process.exitCode = 1;
        return;
    }

    let buildId = buildIdentity(verifiedCompiler());
    let outDir = path.resolve("target");
    let dylib = path.join(outDir, "recon-helper.dylib");
    let buildIdPath = path.join(outDir, "recon-helper.build-id");
    mkdirSync(outDir, { recursive: true });
    rmSync(dylib, { force: true });
    rmSync(buildIdPath, { force: true });

    // Both Apple-silicon slices: a process can only load a dylib of its own architecture, so the
    // helper must match arm64 and arm64e targets alike.
    let universal = [
        "swift",
        "build",
        "--package-path",
        path.resolve("helper"),
        "-c",
        "release",
        "--arch",
        "arm64",
        "--arch",
        "arm64e",
    ];
    execFileSync(
        "xcrun",
        [
            ...universal,
            "-Xcc",
            `-DRECON_HELPER_BUILD="${buildId}"`,
            // Without this the linker's debug map embeds absolute source and object paths, and
            // two checkouts of the same sources produce different bytes.
            "-Xlinker",
            "-S",
            // The product's own name, so a test that links it can find it. An absolute install
            // name would put a checkout path in the artifact.
            "-Xlinker",
            "-install_name",
            "-Xlinker",
            "@rpath/recon-helper.dylib",
        ],
        { stdio: "inherit" },
    );
    let products = execFileSync("xcrun", [...universal, "--show-bin-path"], {
        encoding: "utf8",
    }).trim();
    copyFileSync(path.join(products, "libReconHelper.dylib"), dylib);

    // Ad-hoc is enough to load into a cooperative target; the research profile re-signs the whole
    // inspector, helper included, with the entitlements the platform-binary path needs.
    execFileSync("codesign", ["--force", "--sign", "-", dylib], { stdio: "inherit" });
    writeFileSync(buildIdPath, `${buildId}\n`, { mode: 0o600 });

    console.log(`Built ${path.relative(process.cwd(), dylib)} (arm64, arm64e; ${buildId})`);
}

buildHelper();
