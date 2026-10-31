#!/usr/bin/env node
// Reads one release's entry out of CHANGELOG.md.
//
//   node tools/changelog.ts <version>    print the entry; exit 1 when there is none
//
// Every release needs an entry: `tools/tag-release.ts` refuses to tag a version
// without one, and the `github-release` job in .github/workflows/release.yml
// publishes this output as the release notes.

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { repoRoot } from "./project-version.ts";

/**
 * The body under `## <version>`, the heading `changeset version` writes, up to the
 * next heading of the same or a higher level. `undefined` when the version has no
 * heading or nothing under it.
 */
export function changelogEntry(changelog: string, version: string): string | undefined {
    let lines = changelog.split("\n");
    let heading = lines.findIndex(line => line.trimEnd() === `## ${version}`);
    if (heading === -1) return undefined;

    let end = lines.findIndex((line, index) => index > heading && /^#{1,2} /.test(line));
    let body = lines
        .slice(heading + 1, end === -1 ? undefined : end)
        .join("\n")
        .trim();
    return body === "" ? undefined : body;
}

/** The entry for `version` in this repository's CHANGELOG.md, if the file and entry exist. */
export function repositoryChangelogEntry(version: string): string | undefined {
    let path = join(repoRoot(), "CHANGELOG.md");
    if (!existsSync(path)) return undefined;
    return changelogEntry(readFileSync(path, "utf8"), version);
}

function main(args: string[]): number {
    let version = args[0];
    if (!version) {
        console.error("usage: node tools/changelog.ts <version>");
        return 2;
    }
    let entry = repositoryChangelogEntry(version);
    if (entry === undefined) {
        console.error(`error: CHANGELOG.md has no entry for ${version}.`);
        return 1;
    }
    console.log(entry);
    return 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
