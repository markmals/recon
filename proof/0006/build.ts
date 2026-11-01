// Feasibility proof for proposal.0006: builds the Swift-runtime helper and the universal fixture.
//
//   node proof/0006/build.ts [output-dir] [scratch-dir]
//
// The helper's build identity hashes repository-relative paths and contents, the resolved package
// pins, the exact compiler identification, and the build configuration — never timestamps or the
// checkout location.
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
    copyFileSync,
    existsSync,
    mkdirSync,
    readFileSync,
    readdirSync,
    rmSync,
    statSync,
    writeFileSync,
} from "node:fs";
import path from "node:path";

const PINNED_SWIFT = "swiftlang-6.4.0.34.1";
let root = path.resolve(import.meta.dirname, "../..");
let outDir = path.resolve(process.argv[2] ?? path.join(root, "target/proof-0006"));
let scratch = path.resolve(process.argv[3] ?? path.join(outDir, "swift-build"));

// Code-unit order, not locale order, so identity does not depend on the builder's locale.
function byCodeUnit(left: string, right: string): number {
    return left < right ? -1 : left > right ? 1 : 0;
}

function files(directory: string): string[] {
    return readdirSync(directory)
        .flatMap(entry => {
            let full = path.join(directory, entry);
            return statSync(full).isDirectory() ? files(full) : [full];
        })
        .sort(byCodeUnit);
}

function identityInputs(): string[] {
    let inputs = [
        "sdks/swift/Package.swift",
        "extensions/appkit/Package.swift",
        "extensions/appkit/Package.resolved",
        ...files(path.join(root, "sdks/swift/Sources")).map(file => path.relative(root, file)),
        ...files(path.join(root, "extensions/appkit/Sources")).map(file =>
            path.relative(root, file),
        ),
    ];
    return inputs.filter(input => existsSync(path.join(root, input))).sort(byCodeUnit);
}

let compiler = execFileSync("xcrun", ["swiftc", "--version"], { encoding: "utf8" });
if (!compiler.includes(PINNED_SWIFT)) {
    console.error(
        `Expected the pinned Apple Swift compiler (${PINNED_SWIFT}); found:\n${compiler}`,
    );
    process.exit(1);
}

let hash = createHash("sha256");
for (let input of identityInputs()) {
    hash.update(`${input}\0`);
    hash.update(readFileSync(path.join(root, input)));
    hash.update("\0");
}
// The compiler string alone is not enough: Xcode and the Command Line Tools ship the same
// swiftlang build but different linkers and drivers, and their helpers differ byte for byte.
hash.update(compiler);
// `ld -v` reports on standard error.
hash.update(spawnSync("xcrun", ["ld", "-v"], { encoding: "utf8" }).stderr);
hash.update(execFileSync("xcrun", ["--show-sdk-build-version"], { encoding: "utf8" }));
hash.update("arm64+arm64e|release|macOS27.0|ld -S|ReconProtocolVersion=1");
let buildId = hash.digest("hex");

mkdirSync(outDir, { recursive: true });
execFileSync(
    "xcrun",
    [
        "swift",
        "build",
        "-c",
        "release",
        "--arch",
        "arm64",
        "--arch",
        "arm64e",
        "--package-path",
        path.join(root, "extensions/appkit"),
        "--scratch-path",
        scratch,
        "-Xcc",
        `-DRECON_HELPER_BUILD="${buildId}"`,
        // The linker's debug map (STABS SO/OSO entries) records absolute source and object paths.
        "-Xlinker",
        "-S",
    ],
    { stdio: "inherit" },
);
let helper = path.join(outDir, "recon-helper.dylib");
rmSync(helper, { force: true });
copyFileSync(path.join(scratch, "out/Products/Release/libReconHelper.dylib"), helper);
execFileSync("codesign", ["--force", "--sign", "-", helper], { stdio: "inherit" });
writeFileSync(path.join(outDir, "recon-helper.build-id"), `${buildId}\n`, { mode: 0o600 });

let fixture = path.join(outDir, "proof-fixture");
execFileSync("clang", [
    "-arch",
    "arm64",
    "-arch",
    "arm64e",
    "-fobjc-arc",
    "-O2",
    "-framework",
    "AppKit",
    path.join(root, "proof/0006/fixture/main.m"),
    "-o",
    fixture,
]);
let entitlements = path.join(outDir, "proof-fixture.entitlements.plist");
writeFileSync(
    entitlements,
    `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>com.apple.security.get-task-allow</key><true/></dict></plist>
`,
);
execFileSync("codesign", ["--force", "--sign", "-", "--entitlements", entitlements, fixture]);

console.log(`helper ${helper}\nbuild-id ${buildId}\nfixture ${fixture}`);
