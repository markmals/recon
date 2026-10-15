import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, onTestFinished, test } from "vite-plus/test";

let checkRust = fileURLToPath(new URL("./check-rust.ts", import.meta.url));

test("rejects an unregistered crate while other workspace members exist", () => {
    let root = mkdtempSync(path.join(tmpdir(), "recon-rust-check-"));
    onTestFinished(() => rmSync(root, { force: true, recursive: true }));
    writeFileSync(
        path.join(root, "Cargo.toml"),
        '[workspace]\nresolver = "3"\nmembers = ["crates/registered"]\n',
    );

    for (let name of ["registered", "unregistered"]) {
        let crate = path.join(root, "crates", name);
        mkdirSync(path.join(crate, "src"), { recursive: true });
        writeFileSync(
            path.join(crate, "Cargo.toml"),
            `[package]\nname = "${name}"\nversion = "0.1.0"\nedition = "2024"\n`,
        );
        writeFileSync(path.join(crate, "src", "lib.rs"), "pub fn ready() -> bool { true }\n");
    }

    let check = spawnSync(process.execPath, [checkRust], { cwd: root, encoding: "utf8" });
    expect(check.status).not.toBe(0);
    expect(check.stderr).toContain("crates/unregistered/Cargo.toml is not a workspace member");
});
