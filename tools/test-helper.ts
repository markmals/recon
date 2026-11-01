// The Objective-C runtime support's own tests: framing, tokens, and the authenticated access
// lifecycle. They link the built helper rather than recompiling it, because the server they
// exercise answers reads through the Swift runtime inside that dylib.
//
// Everything the Swift SDK and the AppKit extension own is tested by `mise run swift:test`.
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import path from "node:path";

if (process.platform === "darwin") {
    let helper = path.resolve("target/recon-helper.dylib");
    if (!existsSync(helper)) {
        console.error("Build the helper first: mise run helper:build");
        process.exitCode = 1;
    } else {
        let output = path.resolve("target/helper-tests");
        mkdirSync(output, { recursive: true });
        for (let test of ["helper_tests", "session_tests"]) {
            let executable = path.join(output, test);
            execFileSync(
                "clang",
                [
                    "-fobjc-arc",
                    "-O1",
                    path.resolve(`helper/tests/${test}.m`),
                    helper,
                    // The helper's install name is @rpath-relative, as a package product's is.
                    `-Wl,-rpath,${path.dirname(helper)}`,
                    "-framework",
                    "Foundation",
                    "-lbsm",
                    "-o",
                    executable,
                ],
                { stdio: "inherit" },
            );
            execFileSync(executable, { stdio: "inherit" });
            console.log(`Passed ${test}`);
        }
    }
}
