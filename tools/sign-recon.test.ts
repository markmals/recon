import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, onTestFinished, test } from "vite-plus/test";

let signRecon = fileURLToPath(new URL("./sign-recon.ts", import.meta.url));

test.for([["unset", undefined] as const, ["blank", "   "] as const])(
    "refuses to sign recon when RECON_SIGNING_IDENTITY is %s",
    ([, identity]) => {
        let root = mkdtempSync(path.join(tmpdir(), "recon-sign-"));
        onTestFinished(() => rmSync(root, { force: true, recursive: true }));

        let signing = spawnSync(process.execPath, [signRecon], {
            cwd: root,
            encoding: "utf8",
            env: { ...process.env, RECON_SIGNING_IDENTITY: identity },
        });

        expect(signing.status).not.toBe(0);
        expect(signing.stderr).toContain("RECON_SIGNING_IDENTITY is not set");
        expect(existsSync(path.join(root, "target"))).toBe(false);
    },
);

test("refuses the ad-hoc identity, which signs without a Developer ID", () => {
    let root = mkdtempSync(path.join(tmpdir(), "recon-sign-"));
    onTestFinished(() => rmSync(root, { force: true, recursive: true }));

    let signing = spawnSync(process.execPath, [signRecon], {
        cwd: root,
        encoding: "utf8",
        env: { ...process.env, RECON_SIGNING_IDENTITY: "-" },
    });

    expect(signing.status).not.toBe(0);
    expect(signing.stderr).toContain("ad-hoc signature");
    expect(signing.stderr).toContain("never produces an ad-hoc binary");
    expect(existsSync(path.join(root, "target"))).toBe(false);
});
