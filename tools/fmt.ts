#!/usr/bin/env node
// One formatter entry point for both of recon's languages. With no arguments it formats
// the whole repository; with paths it formats only those files, which is what
// `.omp/extensions/format-after-edit.ts` passes after an edit.
//
//   node tools/fmt.ts
//   node tools/fmt.ts crates/recon/src/main.rs tools/fmt.ts
//
// A path neither formatter owns — recon's C, Objective-C and assembly sources, a plist,
// a fixture binary — is dropped rather than refused, because an edit touches those like
// any other file and the hook has no way to know which formatter, if any, wants them.
//
// Both binaries are already on PATH: mise installs Rust and puts node_modules/.bin there.

import { spawnSync } from "node:child_process";
import path from "node:path";

import { repoRoot } from "./project-version.ts";

/** What Oxfmt parses here. Handed anything else, it fails rather than skipping it. */
const OXFMT_EXTENSIONS = new Set([
    ".ts",
    ".tsx",
    ".js",
    ".mjs",
    ".cjs",
    ".json",
    ".jsonc",
    ".yaml",
    ".yml",
    ".toml",
    ".md",
    ".css",
    ".vue",
]);

export interface Buckets {
    rust: string[];
    oxfmt: string[];
}

/** Splits paths between the two formatters, dropping every path neither one owns. */
export function bucket(paths: string[]): Buckets {
    let buckets: Buckets = { rust: [], oxfmt: [] };
    for (let candidate of paths) {
        let extension = path.extname(candidate).toLowerCase();
        if (extension === ".rs") buckets.rust.push(candidate);
        else if (OXFMT_EXTENSIONS.has(extension)) buckets.oxfmt.push(candidate);
    }
    return buckets;
}

function run(command: string, args: string[], root: string): number {
    return spawnSync(command, args, { cwd: root, stdio: "inherit" }).status ?? 1;
}

function main(argv: string[]): number {
    let root = repoRoot();
    let paths = argv.filter(candidate => candidate.length > 0);

    // Both formatters run even when the first one fails: a formatting failure in one
    // language is no reason to leave the other language unformatted.
    if (paths.length === 0) {
        let oxfmt = run("oxfmt", [], root);
        return run("cargo", ["fmt", "--all"], root) || oxfmt;
    }

    let buckets = bucket(paths);
    // rustfmt finds rustfmt.toml by walking up from each file, the same way `cargo fmt`
    // does, but it has no manifest to read the edition from, so pass it explicitly.
    let rust =
        buckets.rust.length === 0
            ? 0
            : run("rustfmt", ["--edition", "2024", ...buckets.rust], root);
    let oxfmt = buckets.oxfmt.length === 0 ? 0 : run("oxfmt", buckets.oxfmt, root);
    return rust || oxfmt;
}

if (import.meta.main) process.exitCode = main(process.argv.slice(2));
