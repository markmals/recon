// Builds the universal target helper and the build identity the CLI must match.
//
// The identity hashes repository-relative input paths and contents, the resolved package pins,
// this build definition, the compiler, linker and SDK identities, and the build configuration —
// never a timestamp or a checkout path. With the link options below and the UUID stamp, the same
// inputs produce the same signed bytes from any checkout, which is what makes a helper and a
// `recon` build verifiably matched.
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
    copyFileSync,
    existsSync,
    mkdirSync,
    readFileSync,
    readdirSync,
    rmSync,
    writeFileSync,
} from "node:fs";
import path from "node:path";

import { verifiedCompiler } from "./swift-toolchain.ts";

/// What the helper answers with, beyond anything a file states.
const CONFIGURATION = "arm64+arm64e|release|macOS27.0|ReconProtocolVersion=2";

/// Every file that decides what the helper is built from, including this one: the link line
/// lives here, so a change to it is a change to the helper even when no source moved.
const MANIFESTS = [
    "swift-toolchain.toml",
    "tools/build-helper.ts",
    "tools/swift-toolchain.ts",
    "sdks/swift/Package.swift",
    "sdks/swift/Package.resolved",
    "extensions/appkit/Package.swift",
    "extensions/appkit/Package.resolved",
    "extensions/catalyst/Package.swift",
    "extensions/catalyst/Package.resolved",
    "helper/Package.swift",
    "helper/Package.resolved",
];

const SOURCE_TREES = [
    "sdks/swift/Sources",
    "extensions/appkit/Sources",
    "extensions/catalyst/Sources",
    "helper/Sources",
];

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
    hash.update(
        execFileSync("xcrun", ["--sdk", "macosx", "--show-sdk-build-version"], {
            encoding: "utf8",
        }),
    );
    hash.update(CONFIGURATION);
    return hash.digest("hex");
}

/// Replaces every slice's image UUID with one derived from the build identity and the slice's
/// architecture.
///
/// The linker gives each link a fresh UUID rather than one derived from what it linked, so two
/// builds of the same inputs would otherwise differ in those sixteen bytes and in the signature
/// over them. Deriving the UUID from the identity makes the artifact a function of its inputs
/// again; including the architecture keeps the arm64 and arm64e slices distinct, as tools that
/// key images by UUID expect. It runs before signing, so the signature covers what ships.
function stampIdentity(dylib: string, buildId: string): void {
    let image = readFileSync(dylib);
    for (let slice of slices(image)) {
        let architecture = `${image.readUInt32LE(slice + 4)}:${image.readUInt32LE(slice + 8)}`;
        let stamp = createHash("sha256").update(`${buildId}|${architecture}`).digest();
        stampSlice(image, slice, stamp.subarray(0, 16));
    }
    writeFileSync(dylib, image);
}

/** Where each architecture's Mach-O header starts. */
function slices(image: Buffer): number[] {
    let FAT_MAGIC = 0xcafe_babe;
    let MACHO_64_MAGIC = 0xfeed_facf;
    if (image.readUInt32LE(0) === MACHO_64_MAGIC) return [0];
    if (image.readUInt32BE(0) !== FAT_MAGIC) {
        throw new Error("the built helper is neither a 64-bit Mach-O nor a universal file");
    }
    let count = image.readUInt32BE(4);
    return Array.from({ length: count }, (_, index) => image.readUInt32BE(8 + index * 20 + 8));
}

function stampSlice(image: Buffer, header: number, stamp: Buffer): void {
    let LC_UUID = 0x1b;
    let commands = image.readUInt32LE(header + 16);
    let cursor = header + 32;
    for (let index = 0; index < commands; index++) {
        let command = image.readUInt32LE(cursor);
        let size = image.readUInt32LE(cursor + 4);
        if (command === LC_UUID) {
            stamp.copy(image, cursor + 8);
            return;
        }
        cursor += size;
    }
    throw new Error("the built helper has no image UUID to stamp");
}

function buildHelper(args: string[]): void {
    let unknown = args.filter(arg => arg !== "--disable-sandbox");
    if (unknown.length > 0) {
        console.error(
            `usage: node tools/build-helper.ts [--disable-sandbox]; got ${unknown.join(" ")}`,
        );
        process.exitCode = 1;
        return;
    }
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
    // The helper reads its own identity out of this section at run time. It is linked in rather
    // than compiled in because the helper has no C left to compile a definition into.
    let section = path.join(outDir, "recon-helper.identity");
    mkdirSync(outDir, { recursive: true });
    rmSync(dylib, { force: true });
    rmSync(buildIdPath, { force: true });
    // Only when it changes, so an unchanged build does not relink for nothing.
    if (!existsSync(section) || readFileSync(section, "utf8") !== buildId) {
        writeFileSync(section, buildId, { mode: 0o600 });
    }

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
        // A build that already runs inside a sandbox, as Homebrew's does, cannot start SwiftPM's
        // own: macOS refuses the nested sandbox_apply. Output is the same either way.
        ...(args.includes("--disable-sandbox") ? ["--disable-sandbox"] : []),
    ];
    execFileSync(
        "xcrun",
        [
            ...universal,
            "-Xlinker",
            "-sectcreate",
            "-Xlinker",
            "__TEXT",
            "-Xlinker",
            "__recon_build",
            "-Xlinker",
            section,
            // clang's selector stubs make the linker synthesize __objc_stubs, whose order varies
            // from link to link; plain objc_msgSend calls keep the image a function of its inputs.
            "-Xcc",
            "-fno-objc-msgsend-selector-stubs",
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
    stampIdentity(dylib, buildId);

    // Ad-hoc is enough to load into a cooperative target; the research profile re-signs the whole
    // inspector, helper included, with the entitlements the platform-binary path needs.
    execFileSync("codesign", ["--force", "--sign", "-", dylib], { stdio: "inherit" });
    writeFileSync(buildIdPath, `${buildId}\n`, { mode: 0o600 });

    console.log(`Built ${path.relative(process.cwd(), dylib)} (arm64, arm64e; ${buildId})`);
}

buildHelper(process.argv.slice(2));
