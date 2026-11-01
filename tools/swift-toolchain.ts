// The one place Recon's Swift compiler pin lives.
//
// mise cannot provision this toolchain — it is Apple's, shipped by Xcode and the Command Line
// Tools, not swift.org's — so `mise run swift:toolchain` verifies it instead of installing it.
import { execFileSync } from "node:child_process";

/**
 * Apple's Swift 6.4, from Xcode 27.1 or the Command Line Tools. The swift.org toolchain cannot
 * build the helper's arm64e slice: its `usr/lib/swift/macosx` stubs lack arm64e, and the arm64e
 * code it generates dies in the OS runtime with `EXC_ARM_PAC_FAIL`.
 */
export const PINNED_SWIFT = "swiftlang-6.4.0.34.1";

/** The active compiler's identification, or an error naming what was found instead. */
export function verifiedCompiler(): string {
    let compiler = execFileSync("xcrun", ["swiftc", "--version"], { encoding: "utf8" });
    if (!compiler.includes(PINNED_SWIFT)) {
        throw new Error(
            `Recon requires Apple's Swift ${PINNED_SWIFT}, from Xcode 27.1 or the Command Line ` +
                `Tools. The active compiler is:\n${compiler.trim()}`,
        );
    }
    return compiler;
}

if (process.argv[1]?.endsWith("swift-toolchain.ts")) {
    if (process.platform !== "darwin") {
        console.error("Recon's Swift packages build only on macOS.");
        process.exitCode = 1;
    } else {
        process.stdout.write(verifiedCompiler());
    }
}
