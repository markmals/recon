#!/usr/bin/env node
// Decides whether the push that just landed on `main` released a new version, and
// if so creates and pushes its tag.
//
// Run by the `tag` job in .github/workflows/release.yml. It writes `tag` and
// `release` to $GITHUB_OUTPUT; every job that publishes is gated on that job's
// `release` output being `true`.
//
// The tag is not what starts the release — a tag pushed with GITHUB_TOKEN
// does not trigger another workflow. It exists so the GitHub Release, the
// changelog, and `git describe` all point at the same commit.

import { spawnSync } from "node:child_process";
import { appendFileSync } from "node:fs";

import { projectVersion, repoRoot } from "./project-version.ts";

export type Decision = { tag: string; release: boolean; reason: string };

export function decide(version: string, tagExists: boolean): Decision {
    let tag = `v${version}`;
    if (tagExists) {
        return { tag, release: false, reason: `${tag} already exists; no version bump landed.` };
    }
    return { tag, release: true, reason: `Tagging ${tag}.` };
}

function git(root: string, ...args: string[]): number {
    return spawnSync("git", args, { cwd: root, stdio: "inherit" }).status ?? 1;
}

function main(): number {
    let root = repoRoot();
    let version = projectVersion(root);
    let probe = spawnSync("git", ["rev-parse", "-q", "--verify", `refs/tags/v${version}`], {
        cwd: root,
    });
    let decision = decide(version, probe.status === 0);

    console.log(decision.reason);
    let output = process.env.GITHUB_OUTPUT;
    if (output) {
        appendFileSync(output, `tag=${decision.tag}\nrelease=${decision.release}\n`);
    }
    if (!decision.release) return 0;

    let bot = "41898282+github-actions[bot]@users.noreply.github.com";
    for (let args of [
        ["config", "user.name", "github-actions[bot]"],
        ["config", "user.email", bot],
        ["tag", "-a", decision.tag, "-m", decision.tag],
        ["push", "origin", decision.tag],
    ]) {
        let status = git(root, ...args);
        if (status !== 0) return status;
    }
    return 0;
}

if (import.meta.main) process.exit(main());
