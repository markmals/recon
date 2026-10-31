import assert from "node:assert/strict";
import { test } from "node:test";

import { bucket } from "./fmt.ts";

test("routes Rust sources to rustfmt and everything Oxfmt parses to oxfmt", () => {
    assert.deepEqual(
        bucket([
            "crates/recon/src/main.rs",
            "tools/fmt.ts",
            "docs/.vitepress/config.ts",
            "package.json",
            ".oxfmtrc.jsonc",
            "mise.toml",
            "README.md",
            "docs/index.css",
            "docs/components/Demo.vue",
            ".github/workflows/ci.yml",
        ]),
        {
            rust: ["crates/recon/src/main.rs"],
            oxfmt: [
                "tools/fmt.ts",
                "docs/.vitepress/config.ts",
                "package.json",
                ".oxfmtrc.jsonc",
                "mise.toml",
                "README.md",
                "docs/index.css",
                "docs/components/Demo.vue",
                ".github/workflows/ci.yml",
            ],
        },
    );
});

test("drops a path neither formatter owns instead of failing on it", () => {
    // An edit to the loader or the helper passes these in; recon has no formatter for them.
    assert.deepEqual(
        bucket([
            "loader/loader.c",
            "helper/main.m",
            "crates/recon/src/trampoline.S",
            "helper/Info.plist",
            "LICENSE",
        ]),
        { rust: [], oxfmt: [] },
    );
});

test("matches extensions regardless of case", () => {
    assert.deepEqual(bucket(["Notes.MD", "crates/recon/build.RS"]), {
        rust: ["crates/recon/build.RS"],
        oxfmt: ["Notes.MD"],
    });
});

test("leaves both buckets empty for no paths, so neither formatter runs", () => {
    assert.deepEqual(bucket([]), { rust: [], oxfmt: [] });
});

test("keeps the order the caller passed", () => {
    assert.deepEqual(bucket(["b.ts", "a.rs", "a.ts", "b.rs"]), {
        rust: ["a.rs", "b.rs"],
        oxfmt: ["b.ts", "a.ts"],
    });
});
