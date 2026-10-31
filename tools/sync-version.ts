#!/usr/bin/env node
// Copies the version Changesets wrote into package.json onto the Cargo workspace,
// then refreshes Cargo.lock so the lockfile and the manifest agree.
//
//   node tools/sync-version.ts            write the version into Cargo.toml
//   node tools/sync-version.ts --check    exit 1 when the two have drifted
//
// `mise run version` runs this immediately after `changeset version`. Nothing else
// writes `[workspace.package] version`, which is what makes `--check` a usable gate.

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { projectVersion, repoRoot } from "./project-version.ts";

/**
 * Locate the `version = "…"` line inside `[workspace.package]`.
 *
 * A regex over the whole file would also match `[package] version`, a dependency's
 * `version`, or a commented-out line, so walk the one table instead.
 */
export function findWorkspaceVersion(manifest: string): { line: number; current: string } {
    let lines = manifest.split("\n");
    let header = lines.findIndex(line => line.trim() === "[workspace.package]");
    if (header === -1) throw new Error("Cargo.toml has no [workspace.package] table");

    for (let i = header + 1; i < lines.length; i++) {
        let line = lines[i] ?? "";
        if (line.trimStart().startsWith("[")) break; // next table; the version is missing
        let match = /^version\s*=\s*"([^"]*)"/.exec(line);
        if (match) return { line: i, current: match[1] ?? "" };
    }
    throw new Error('[workspace.package] has no `version = "…"` line');
}

/** The manifest with `[workspace.package] version` set to `version`. */
export function withWorkspaceVersion(manifest: string, version: string): string {
    let { line } = findWorkspaceVersion(manifest);
    let lines = manifest.split("\n");
    lines[line] = `version = "${version}"`;
    return lines.join("\n");
}

function main(args: string[]): number {
    let check = args.includes("--check");
    let root = repoRoot();
    let manifestPath = join(root, "Cargo.toml");

    let version = projectVersion(root);
    let manifest = readFileSync(manifestPath, "utf8");
    let { current } = findWorkspaceVersion(manifest);

    if (current === version) {
        console.log(`Cargo workspace already at ${version}.`);
        return 0;
    }
    if (check) {
        console.error(
            `error: package.json is ${version} but Cargo.toml is ${current}. Run \`mise run version\`.`,
        );
        return 1;
    }

    writeFileSync(manifestPath, withWorkspaceVersion(manifest, version));
    console.log(`Cargo workspace ${current} -> ${version}.`);

    // `--workspace` touches only the member entries, so a version bump never
    // silently upgrades a dependency. `--offline` keeps it off the registry.
    execFileSync("cargo", ["update", "--workspace", "--offline", "--quiet"], {
        cwd: root,
        stdio: "inherit",
    });
    return 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
