import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, renameSync, rmSync } from "node:fs";
import path from "node:path";

const IDENTIFIER = "me.malstrom.recon";

function signRecon(): void {
    let identity = process.env.RECON_SIGNING_IDENTITY?.trim();
    if (!identity) {
        console.error("RECON_SIGNING_IDENTITY is not set.");
        console.error(
            "Recon's Screen Recording and Accessibility grants belong to a stable Developer ID signature, so this task never produces an ad-hoc binary.",
        );
        console.error(
            "Set the identity, for example: RECON_SIGNING_IDENTITY='Developer ID Application: Your Name (TEAMID)' vp run recon:sign",
        );
        process.exitCode = 1;
        return;
    }

    let signed = path.resolve("target/signed");
    let staged = path.join(signed, "recon.staged");
    let recon = path.join(signed, "recon");

    execFileSync("cargo", ["build", "--release", "-p", "recon"], { stdio: "inherit" });
    mkdirSync(signed, { recursive: true });
    copyFileSync(path.resolve("target/release/recon"), staged);

    try {
        execFileSync(
            "codesign",
            ["--force", "--sign", identity, "--identifier", IDENTIFIER, staged],
            { stdio: "inherit" },
        );
        // Replacing the file by rename keeps macOS from seeing a signed executable change
        // underneath a process that is already running it, which invalidates its signature.
        renameSync(staged, recon);
    } finally {
        rmSync(staged, { force: true });
    }

    execFileSync("codesign", ["-dr", "-", recon], { stdio: "inherit" });
    console.log(`Signed ${path.relative(process.cwd(), recon)} as ${IDENTIFIER}`);
}

signRecon();
