import { execFileSync, spawnSync, type SpawnSyncReturns } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

let manifest = fileURLToPath(new URL("../crates/recon/Cargo.toml", import.meta.url));

// `vp run check` runs these tests alongside `cargo test`, which executes target/debug/recon.
// Every cargo build re-links that file, even when nothing changed, and a recon that re-executes
// itself in that moment finds no executable; builds also wait on each other's lock. A target
// directory of their own keeps the two suites apart.
let targetDirectory = fileURLToPath(new URL("../target/tools", import.meta.url));
let executable = path.join(targetDirectory, "debug", "recon");

export function buildRecon(): void {
    execFileSync("cargo", [
        "build",
        "--quiet",
        "--manifest-path",
        manifest,
        "--target-dir",
        targetDirectory,
    ]);
}

export function recon(
    args: string[],
    options: { timeout?: number } = {},
): SpawnSyncReturns<string> {
    return spawnSync(executable, args, { encoding: "utf8", ...options });
}
