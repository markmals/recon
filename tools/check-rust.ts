import { execFileSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import path from "node:path";

interface CargoMetadata {
    packages: { id: string; manifest_path: string }[];
    workspace_members: string[];
}

function checkRust(): void {
    let metadata = JSON.parse(
        execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
            encoding: "utf8",
        }),
    ) as CargoMetadata;
    let workspaceMembers = new Set(metadata.workspace_members);
    let memberManifests = new Set(
        metadata.packages
            .filter(pkg => workspaceMembers.has(pkg.id))
            .map(pkg => path.resolve(pkg.manifest_path)),
    );
    let cratesRoot = path.resolve("crates");

    for (let crate of existsSync(cratesRoot)
        ? readdirSync(cratesRoot, { withFileTypes: true })
        : []) {
        if (!crate.isDirectory()) continue;
        let manifest = path.join(cratesRoot, crate.name, "Cargo.toml");
        if (!existsSync(manifest) || memberManifests.has(manifest)) continue;
        console.error(
            `${path.relative(process.cwd(), manifest)} is not a workspace member; add it to Cargo.toml`,
        );
        process.exitCode = 1;
        return;
    }

    if (workspaceMembers.size === 0) {
        console.log("No Rust workspace members yet; Rust checks skipped.");
        return;
    }

    for (let args of [
        ["fmt", "--all", "--check"],
        ["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"],
        ["test", "--workspace"],
    ]) {
        execFileSync("cargo", args, { stdio: "inherit" });
    }
}

checkRust();
