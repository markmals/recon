#!/usr/bin/env node
// Checks the one Rust property nothing else checks: every directory under `crates/`
// that carries a Cargo.toml is a member of the workspace. A crate the workspace does
// not claim is built by nothing and checked by nothing, so it rots unnoticed.
//
// `mise run crates` runs this. `cargo fmt --check`, `cargo clippy -D warnings`,
// `cargo test --workspace` and `cargo doc` used to run from here too; they are hk
// steps and mise tasks of their own now (`mise run fmt`, `lint`, `test`, `doc`), and
// leaving them here would run every one of them twice in `mise run check`.

import { execFileSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import path from "node:path";

interface CargoMetadata {
    packages: { id: string; manifest_path: string }[];
    workspace_members: string[];
}

/** Manifests under `crates/` that the workspace does not list as members. */
export function unregisteredCrates(root: string): string[] {
    let metadata = JSON.parse(
        execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
            cwd: root,
            encoding: "utf8",
        }),
    ) as CargoMetadata;
    let workspaceMembers = new Set(metadata.workspace_members);
    let memberManifests = new Set(
        metadata.packages
            .filter(pkg => workspaceMembers.has(pkg.id))
            .map(pkg => path.resolve(pkg.manifest_path)),
    );

    let cratesRoot = path.resolve(root, "crates");
    if (!existsSync(cratesRoot)) return [];
    return (
        readdirSync(cratesRoot, { withFileTypes: true })
            .filter(entry => entry.isDirectory())
            .map(entry => path.join(cratesRoot, entry.name, "Cargo.toml"))
            // A directory without a manifest is not a crate, so it cannot be a member.
            .filter(manifest => existsSync(manifest) && !memberManifests.has(manifest))
    );
}

function main(): number {
    let root = process.cwd();
    let unregistered = unregisteredCrates(root);
    for (let manifest of unregistered) {
        console.error(
            `${path.relative(root, manifest)} is not a workspace member; add it to Cargo.toml`,
        );
    }
    return unregistered.length === 0 ? 0 : 1;
}

if (import.meta.main) process.exitCode = main();
