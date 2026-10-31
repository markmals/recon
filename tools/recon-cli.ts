import { execFileSync, spawnSync, type SpawnSyncReturns } from "node:child_process";
import { fileURLToPath } from "node:url";

let manifest = fileURLToPath(new URL("../crates/recon/Cargo.toml", import.meta.url));

// `mise run check` runs these tests alongside `cargo test`, which executes target/debug/recon.
// Every cargo build re-links that file, even when nothing changed, and a recon that re-executes
// itself in that moment finds no executable; builds also wait on each other's lock. A target
// directory of their own keeps the two suites apart.
let targetDirectory = fileURLToPath(new URL("../target/tools", import.meta.url));
let executable: string;

export function buildRecon(): void {
    let output = execFileSync(
        "cargo",
        [
            "build",
            "--quiet",
            "--manifest-path",
            manifest,
            "--target-dir",
            targetDirectory,
            "--message-format=json-render-diagnostics",
        ],
        { encoding: "utf8" },
    );
    for (let line of output.trim().split("\n")) {
        let message = JSON.parse(line);
        if (
            message.reason === "compiler-artifact" &&
            message.target.name === "recon" &&
            typeof message.executable === "string"
        ) {
            executable = message.executable;
            return;
        }
    }
    throw new Error("Cargo did not report the recon executable");
}

export function recon(
    args: string[],
    options: { timeout?: number } = {},
): SpawnSyncReturns<string> {
    return spawnSync(executable, args, { encoding: "utf8", ...options });
}
