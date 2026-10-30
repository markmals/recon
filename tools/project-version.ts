// Where the project's version lives.
//
// Changesets owns the `version` field in the root `package.json`. Every script that
// needs the version reads it from here, so none of them can disagree about what
// "the version" means, and the parsing can be tested on its own.
//
// The root is resolved from this file rather than from the process's directory, so
// a script behaves the same whether a task, a hook, or a person at a terminal in a
// subdirectory started it.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** The repository root: the directory holding `tools/`. */
export function repoRoot(): string {
    return dirname(dirname(fileURLToPath(import.meta.url)));
}

/** The version Changesets wrote, read out of a `package.json` body. */
export function versionFrom(packageJson: string): string {
    let pkg: unknown = JSON.parse(packageJson);
    if (pkg && typeof pkg === "object" && "version" in pkg) {
        let version = pkg.version;
        if (typeof version === "string" && version.length > 0) return version;
    }
    throw new Error('package.json has no "version"');
}

/** The version Changesets wrote, read from the project's `package.json`. */
export function projectVersion(root: string = repoRoot()): string {
    return versionFrom(readFileSync(join(root, "package.json"), "utf8"));
}
