import { execFileSync, spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, renameSync, rmSync } from "node:fs";
import path from "node:path";

const IDENTIFIER = "me.malstrom.recon";

function refuse(reason: string): void {
    console.error(reason);
    console.error(
        "Recon's Screen Recording and Accessibility grants belong to a stable Developer ID signature, so this task never produces an ad-hoc binary.",
    );
    console.error(
        "Set the identity, for example: RECON_SIGNING_IDENTITY='Developer ID Application: Your Name (TEAMID)' mise run recon:sign",
    );
    process.exitCode = 1;
}

/**
 * Describes why a signature would lose its grants on the next rebuild, or nothing when it keeps
 * them: an ad-hoc signature's designated requirement is the code hash itself.
 */
function adHocSignature(binary: string): string | undefined {
    // `codesign -d` writes its description to standard error.
    let inspection = spawnSync("codesign", ["-dv", binary], { encoding: "utf8" });
    let description = `${inspection.stdout}${inspection.stderr}`;

    if (/flags=\S*adhoc/.test(description)) return "codesign reported an ad-hoc signature";
    let team = /^TeamIdentifier=(.*)$/m.exec(description)?.[1].trim();
    if (!team || team === "not set") return "the signature names no team";
    return undefined;
}

function signRecon(): void {
    let identity = process.env.RECON_SIGNING_IDENTITY?.trim();
    if (!identity) {
        refuse("RECON_SIGNING_IDENTITY is not set.");
        return;
    }
    if (identity === "-") {
        refuse('RECON_SIGNING_IDENTITY is "-", which asks codesign for an ad-hoc signature.');
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
        let refusal = adHocSignature(staged);
        if (refusal) {
            refuse(`${path.relative(process.cwd(), recon)} was left as it was: ${refusal}.`);
            return;
        }
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
