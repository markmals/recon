#!/usr/bin/env node
// Checks the two Rust properties nothing else checks: every directory under `crates/`
// that carries a Cargo.toml is a member of the workspace, and the workspace's crates
// point the way the architecture says they do. A crate the workspace does not claim is
// built by nothing and checked by nothing, so it rots unnoticed; a dependency pointing
// the wrong way compiles perfectly and quietly undoes the ownership boundaries.
//
// `mise run crates` runs this. `cargo fmt --check`, `cargo clippy -D warnings`,
// `cargo test --workspace` and `cargo doc` used to run from here too; they are hk
// steps and mise tasks of their own now (`mise run fmt`, `lint`, `test`, `doc`), and
// leaving them here would run every one of them twice in `mise run check`.

import { execFileSync } from "node:child_process";
import { existsSync, readdirSync } from "node:fs";
import path from "node:path";

interface CargoDependency {
    name: string;
    kind: string | null;
    /** The `cfg(…)` or triple the dependency is declared under, and null for every target. */
    target: string | null;
}

interface CargoPackage {
    id: string;
    name: string;
    manifest_path: string;
    dependencies: CargoDependency[];
}

interface CargoMetadata {
    packages: CargoPackage[];
    workspace_members: string[];
}

/** The executable crate: it assembles the product, so nothing may depend on it. */
const EXECUTABLE = "recon";

/** The portable core: it owns the shared contracts, so it depends on no sibling. */
const CORE = "recon-core";

/**
 * Apple's framework bindings. A `-macos` crate that declares one for every target cannot
 * build on the Linux gate, which builds and tests every workspace member.
 */
const APPLE_BINDINGS = /^(objc2(-|$)|block2$|plist$)/;

function metadata(root: string): CargoMetadata {
    return JSON.parse(
        execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
            cwd: root,
            encoding: "utf8",
        }),
    ) as CargoMetadata;
}

function members(workspace: CargoMetadata): CargoPackage[] {
    let ids = new Set(workspace.workspace_members);
    return workspace.packages.filter(member => ids.has(member.id));
}

/** Manifests under `crates/` that the workspace does not list as members. */
export function unregisteredCrates(root: string): string[] {
    let memberManifests = new Set(
        members(metadata(root)).map(member => path.resolve(member.manifest_path)),
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

/** A dependency that contradicts the workspace's declared direction, as the message to print. */
export function misdirectedDependencies(root: string): string[] {
    let workspace = members(metadata(root));
    let names = new Set(workspace.map(member => member.name));
    let violations: string[] = [];

    for (let member of workspace) {
        let platform = member.name.endsWith("-macos");
        for (let dependency of member.dependencies) {
            if (names.has(dependency.name)) {
                violations.push(...misdirected(member.name, dependency.name, platform));
            } else if (platform && dependency.target === null) {
                violations.push(...ungated(member.name, dependency));
            }
        }
    }
    return violations;
}

function misdirected(from: string, to: string, platform: boolean): string[] {
    if (to === EXECUTABLE) {
        return [
            `${from} depends on ${EXECUTABLE}; nothing depends on the executable, which ` +
                `assembles the product over the crates below it`,
        ];
    }
    if (from === CORE) {
        return [
            `${CORE} depends on ${to}; the portable core owns the shared contracts and ` +
                `depends on no other crate in this workspace`,
        ];
    }
    if (to.endsWith("-macos") && !platform && from !== EXECUTABLE) {
        return [
            `${from} depends on ${to}; only the executable and other platform crates may ` +
                `depend on a platform implementation`,
        ];
    }
    return [];
}

function ungated(from: string, dependency: CargoDependency): string[] {
    if (!APPLE_BINDINGS.test(dependency.name)) return [];
    return [
        `${from} depends on ${dependency.name} for every target; a -macos crate declares its ` +
            `Apple framework bindings under [target.'cfg(target_os = "macos")'.dependencies] so ` +
            `that the workspace still builds where macOS is not the host`,
    ];
}

function main(): number {
    let root = process.cwd();
    let unregistered = unregisteredCrates(root);
    for (let manifest of unregistered) {
        console.error(
            `${path.relative(root, manifest)} is not a workspace member; add it to Cargo.toml`,
        );
    }
    let misdirections = misdirectedDependencies(root);
    for (let violation of misdirections) console.error(violation);
    return unregistered.length === 0 && misdirections.length === 0 ? 0 : 1;
}

if (import.meta.main) process.exitCode = main();
