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

const FENCE = /^ {0,3}(`{3,}|~{3,})/;
// Any ATX heading, which CommonMark allows up to three spaces in. Only the column-0
// `#`/`##` headings Changesets writes end an entry: an indented one belongs to a
// summary's list item.
const HEADING = /^ {0,3}#{1,6}(\s|$)/;
const SECTION = /^#{1,2}(\s|$)/;

/**
 * The body under `## <version>`, the heading `changeset version` writes, up to the
 * next heading of the same or a higher level. Headings inside fenced code blocks are
 * example text, not structure. `undefined` when the version has no heading, or
 * nothing under it but subheadings.
 */
export function changelogEntry(changelog: string, version: string): string | undefined {
    let lines = changelog.split("\n");
    let structural = outsideFences(lines);
    let heading = lines.findIndex(
        (line, index) => structural[index] && line.trimEnd() === `## ${version}`,
    );
    if (heading === -1) return undefined;

    let end = lines.findIndex(
        (line, index) => index > heading && structural[index] && SECTION.test(line),
    );
    let stop = end === -1 ? lines.length : end;
    let body = lines.slice(heading + 1, stop);
    let hasChanges = body.some(
        (line, offset) =>
            line.trim() !== "" && !(structural[heading + 1 + offset] && HEADING.test(line)),
    );
    return hasChanges ? body.join("\n").trim() : undefined;
}

/** For each line, whether it lies outside every fenced code block, fences included. */
function outsideFences(lines: readonly string[]): boolean[] {
    let open: string | undefined;
    return lines.map(line => {
        let run = FENCE.exec(line)?.[1];
        if (open === undefined) {
            if (run !== undefined) open = run;
            return run === undefined;
        }
        let closes = run !== undefined && run[0] === open[0] && run.length >= open.length;
        if (closes && line.trim() === run) open = undefined;
        return false;
    });
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
