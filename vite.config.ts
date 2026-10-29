import { defineConfig } from "vite-plus";

export default defineConfig({
    test: {
        include: ["tools/**/*.test.ts", ".omp/tools/**/*.test.ts"],
    },
    fmt: {
        ignorePatterns: ["target/**"],
        printWidth: 100,
        tabWidth: 4,
        arrowParens: "avoid",
        sortPackageJson: true,
        sortImports: {
            groups: [
                "type-import",
                ["value-builtin", "value-external"],
                "type-internal",
                "value-internal",
                ["type-parent", "type-sibling", "type-index"],
                ["value-parent", "value-sibling", "value-index"],
                "unknown",
            ],
            partitionByComment: true,
        },
        overrides: [
            {
                files: ["**/*.md", "**/*.mdx"],
                options: { proseWrap: "never" },
            },
            {
                files: ["**/*.jsonc"],
                options: { trailingComma: "none" },
            },
        ],
    },
    lint: {
        ignorePatterns: ["target/**"],
        options: {
            typeAware: true,
            typeCheck: true,
        },
        jsPlugins: [
            {
                name: "vite-plus",
                specifier: "vite-plus/oxlint-plugin",
            },
            "eslint-plugin-perfectionist",
            "eslint-plugin-prefer-let",
        ],
        rules: {
            "vite-plus/prefer-vite-plus-imports": "error",
            "typescript/no-floating-promises": "allow",
            "typescript/unbound-method": "allow",
            "perfectionist/sort-jsx-props": "warn",
            "import/extensions": [
                "error",
                "ignorePackages",
                {
                    cjs: "always",
                    cts: "always",
                    js: "always",
                    jsx: "always",
                    mjs: "always",
                    mts: "always",
                    ts: "always",
                    tsx: "always",
                },
            ],
            "eslint/prefer-const": "off",
            "prefer-let/prefer-let": [2, { forceUpperCaseConst: true }],
        },
    },
    run: {
        tasks: {
            check: {
                command: "vp check",
                dependsOn: ["validate", "test", "docs:build", "rust"],
            },
            rust: {
                command: "node tools/check-rust.ts",
                cache: false,
            },
            validate: "node tools/validate.ts",
            test: "vp test run",
            // These three observe the live Mac: the signing keychain, a built app bundle, and a
            // running fixture. Disabling the cache also lifts the task runner's environment
            // filter, so RECON_SIGNING_IDENTITY reaches the signing script.
            "fixture:build": {
                command: "node tools/build-fixture.ts",
                cache: false,
            },
            "fixture:inject": {
                command: "node tools/build-inject-fixture.ts",
                cache: false,
            },
            "helper:build": {
                command: "node tools/build-helper.ts",
                cache: false,
            },
            "loader:build": {
                command: "node tools/build-loader.ts",
                cache: false,
            },
            "recon:sign": {
                command: "node tools/sign-recon.ts",
                cache: false,
            },
            "test:ui": {
                command: "cargo test -p recon --test ui_observation -- --ignored --test-threads=1",
                dependsOn: ["fixture:build", "recon:sign"],
                cache: false,
            },
            // The no-focus evidence: a continuous out-of-process watch over a live run. It
            // activates nothing itself, so run it hands-off.
            "test:focus": {
                command:
                    "cargo test -p recon --test ui_focus reading_ -- --ignored --test-threads=1",
                dependsOn: ["fixture:build", "recon:sign"],
                cache: false,
            },
            "test:inject": {
                command: "node tools/test-inject.ts",
                dependsOn: ["fixture:inject", "loader:build"],
                cache: false,
            },
            "test:helper": {
                command: "node tools/test-helper.ts",
                dependsOn: ["helper:build"],
                cache: false,
            },
            "test:inspection": {
                command:
                    "cargo test -p recon --test inspection_live --test inspection_appkit -- --ignored --test-threads=1",
                dependsOn: ["fixture:build", "fixture:inject", "helper:build", "loader:build"],
                cache: false,
            },
            // Reads live git and GitHub state, so a replayed result would be stale.
            status: {
                command: "node tools/status.ts",
                cache: false,
            },
            "docs:dev": {
                command: "vitepress dev docs",
                cache: false,
            },
            // Tasks run in a filtered environment; DOCS_BASE must be listed to reach VitePress.
            "docs:build": {
                command: "vitepress build docs",
                cache: { env: ["DOCS_BASE"] },
            },
            "docs:preview": {
                command: "vitepress preview docs",
                dependsOn: ["docs:build"],
                cache: false,
            },
        },
    },
});
