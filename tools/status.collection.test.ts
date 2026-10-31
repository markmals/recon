import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test, type TestContext } from "node:test";

let statusScript = new URL("./status.ts", import.meta.url).pathname;

// A git hook exports GIT_DIR, GIT_INDEX_FILE and GIT_WORK_TREE, and a child process
// inherits them. `mise run check` runs these tests from hk's `pre-push` hook, so that
// is the normal case: without stripping those variables, every `git` call below — and
// `status.ts` itself, which shells out to git — would operate on the repository the
// hook is running in instead of on the temporary fixture, and would write commits,
// branches and config into it.
function envWithoutGit(overrides: Record<string, string> = {}): NodeJS.ProcessEnv {
    let env: NodeJS.ProcessEnv = {};
    for (let [name, value] of Object.entries(process.env)) {
        if (!name.startsWith("GIT_")) env[name] = value;
    }
    return { ...env, ...overrides };
}

/** `git`, run against the fixture only, never against the surrounding repository. */
function git(...args: string[]): void {
    execFileSync("git", args, { env: envWithoutGit() });
}

function statusWithPullRequests(t: TestContext, pullRequests: object[]): string {
    let root = mkdtempSync(path.join(tmpdir(), "recon-status-"));
    t.after(() => rmSync(root, { force: true, recursive: true }));
    git("init", "-q", "-b", "feature", root);
    git(
        "-C",
        root,
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "--allow-empty",
        "-qm",
        "fixture",
    );
    let bin = path.join(root, "bin");
    mkdirSync(bin);
    let gh = path.join(bin, "gh");
    writeFileSync(
        gh,
        [
            `#!${process.execPath}`,
            "let command = process.argv[2];",
            'if (command === "--version" || command === "auth") process.exit(0);',
            'if (command === "repo") console.log("main");',
            'else if (command === "pr" && process.argv[3] === "list") {',
            '  let state = process.argv.indexOf("--state");',
            '  let head = process.argv.indexOf("--head");',
            '  console.log(process.argv[state + 1] === "all" && process.argv[head + 1] === "feature" ? process.env.TEST_PULL_REQUESTS : "[]");',
            "}",
            "else process.exit(1);",
        ].join("\n"),
    );
    chmodSync(gh, 0o755);
    mkdirSync(path.join(root, "proposals"));
    writeFileSync(
        path.join(root, "proposals", "0001-feature.md"),
        "---\nid: proposal.0001\ntitle: Feature\nstatus: implemented\npull-request: 1\n---\n",
    );
    return execFileSync(process.execPath, [statusScript], {
        cwd: root,
        encoding: "utf8",
        env: envWithoutGit({
            PATH: `${bin}${path.delimiter}${process.env.PATH ?? ""}`,
            TEST_PULL_REQUESTS: JSON.stringify(pullRequests),
        }),
    });
}

function assertContains(report: string, expected: string): void {
    assert.ok(report.includes(expected), `expected report to contain ${expected}:\n${report}`);
}

test("finds a merged feature PR after it leaves the open PR list", t => {
    let report = statusWithPullRequests(t, [
        {
            number: 1,
            state: "MERGED",
            baseRefName: "main",
            isDraft: false,
            title: "Feature",
            url: "https://github.com/owner/repo/pull/1",
        },
    ]);

    assertContains(report, "PR #1 (merged)");
    assertContains(report, "Phase 5 — Implementation merged");
});

test("does not select an arbitrary PR when a branch name has been reused", t => {
    let report = statusWithPullRequests(t, [
        { number: 1, state: "MERGED", baseRefName: "main", isDraft: false, title: "Earlier" },
        { number: 2, state: "CLOSED", baseRefName: "main", isDraft: false, title: "Later" },
    ]);

    assertContains(report, "PR — unknown (Multiple pull requests found for branch feature");
    assertContains(report, "inspect their proposal links");
    assertContains(report, "Check gh access and the branch's pull request association");
    assert.ok(
        !report.includes("Implementation merged"),
        `expected report to omit "Implementation merged":\n${report}`,
    );
});

function statusInRepositoryWithoutRemote(t: TestContext): string {
    let root = mkdtempSync(path.join(tmpdir(), "recon-status-local-"));
    t.after(() => rmSync(root, { force: true, recursive: true }));
    git("init", "-q", "-b", "main", root);
    git(
        "-C",
        root,
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "--allow-empty",
        "-qm",
        "fixture",
    );
    git("-C", root, "switch", "-q", "-c", "feature/x");

    // No origin and no usable gh, so refs/heads/main is the only evidence of which
    // branch is the default one.
    let bin = path.join(root, "bin");
    mkdirSync(bin);
    let gh = path.join(bin, "gh");
    writeFileSync(gh, `#!${process.execPath}\nprocess.exit(1);\n`);
    chmodSync(gh, 0o755);

    return execFileSync(process.execPath, [statusScript], {
        cwd: root,
        encoding: "utf8",
        env: envWithoutGit({
            PATH: `${bin}${path.delimiter}${process.env.PATH ?? ""}`,
        }),
    });
}

test("recognizes a local default branch when there is no remote and no gh", t => {
    let report = statusInRepositoryWithoutRemote(t);

    assertContains(report, "branch feature/x");
    assertContains(report, "PR — unknown (gh is unavailable)");
    assertContains(report, "Phase 1 — Preparation");
    assert.ok(
        !report.includes("Branch role unknown"),
        `expected report to omit "Branch role unknown":\n${report}`,
    );
});
