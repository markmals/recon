import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test, type TestContext } from "node:test";
import { fileURLToPath } from "node:url";

let signRecon = fileURLToPath(new URL("./sign-recon.ts", import.meta.url));

function signIn(t: TestContext, identity: string | undefined) {
    let root = mkdtempSync(path.join(tmpdir(), "recon-sign-"));
    t.after(() => rmSync(root, { force: true, recursive: true }));

    let signing = spawnSync(process.execPath, [signRecon], {
        cwd: root,
        encoding: "utf8",
        env: { ...process.env, RECON_SIGNING_IDENTITY: identity },
    });

    return { signing, root };
}

for (let [kind, identity] of [
    ["unset", undefined],
    ["blank", "   "],
] as const) {
    test(`refuses to sign recon when RECON_SIGNING_IDENTITY is ${kind}`, t => {
        let { signing, root } = signIn(t, identity);

        assert.notEqual(signing.status, 0);
        assert.match(signing.stderr, /RECON_SIGNING_IDENTITY is not set/);
        assert.equal(existsSync(path.join(root, "target")), false);
    });
}

test("refuses the ad-hoc identity, which signs without a Developer ID", t => {
    let { signing, root } = signIn(t, "-");

    assert.notEqual(signing.status, 0);
    assert.match(signing.stderr, /ad-hoc signature/);
    assert.match(signing.stderr, /never produces an ad-hoc binary/);
    assert.equal(existsSync(path.join(root, "target")), false);
});

test("names the task that signs recon when it refuses", t => {
    let { signing } = signIn(t, undefined);

    assert.match(signing.stderr, /mise run recon:sign/);
});
