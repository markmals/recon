import { spawnSync } from "node:child_process";

let result = spawnSync(
    "cargo",
    [
        "test",
        "-p",
        "recon",
        "--test",
        "inject_binding",
        "--test",
        "inject_loader",
        "--",
        "--ignored",
        "--test-threads=1",
    ],
    { cwd: process.cwd(), stdio: "inherit" },
);
if (result.status !== 0) process.exit(result.status ?? 1);
