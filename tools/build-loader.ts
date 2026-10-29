import { execFileSync } from "node:child_process";
import { mkdirSync, rmSync } from "node:fs";
import path from "node:path";

function buildLoader(): void {
    if (process.platform !== "darwin") {
        console.error("The Recon loader builds only on macOS with the Xcode command-line tools.");
        process.exitCode = 1;
        return;
    }

    let output = path.resolve("target/recon-loader");
    mkdirSync(path.dirname(output), { recursive: true });
    rmSync(output, { force: true });

    execFileSync(
        "clang",
        [
            "-arch",
            "arm64",
            "-arch",
            "arm64e",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            path.resolve("loader/recon_loader.c"),
            path.resolve("helper/bootstrap.s"),
            "-lbsm",
            "-o",
            output,
        ],
        { stdio: "inherit" },
    );
    execFileSync(
        "codesign",
        [
            "--force",
            "--sign",
            "-",
            "--entitlements",
            path.resolve("loader/entitlements.plist"),
            output,
        ],
        { stdio: "inherit" },
    );
    console.log(`Built ${path.relative(process.cwd(), output)} (arm64, arm64e)`);
}

buildLoader();
