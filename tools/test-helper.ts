import { execFileSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import path from "node:path";

if (process.platform === "darwin") {
    let output = path.resolve("target/helper-tests");
    mkdirSync(output, { recursive: true });
    let sources = [
        "helper/recon_helper.c",
        "helper/ReconCore.m",
        "helper/ReconFields.m",
        "helper/ReconFacets.m",
        "helper/ReconInspect.m",
        "helper/ReconServer.m",
    ].map((source) => path.resolve(source));
    for (let test of ["helper_tests", "session_tests", "inspection_tests"]) {
        let executable = path.join(output, test);
        execFileSync(
            "clang",
            [
                "-fobjc-arc",
                "-O1",
                '-DRECON_HELPER_BUILD="fixture"',
                ...sources,
                path.resolve(`helper/tests/${test}.m`),
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
                executable,
            ],
            { stdio: "inherit" },
        );
        execFileSync(executable, { stdio: "inherit" });
        console.log(`Passed ${test}`);
    }
}
