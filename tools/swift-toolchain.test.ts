import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { test, type TestContext } from "node:test";
import { fileURLToPath } from "node:url";

import { PINNED_SWIFT, pinnedSwift } from "./swift-toolchain.ts";

function pinFile(t: TestContext, contents: string): string {
    let directory = mkdtempSync(path.join(tmpdir(), "recon-swift-pin-"));
    t.after(() => rmSync(directory, { force: true, recursive: true }));
    let file = path.join(directory, "swift-toolchain.toml");
    writeFileSync(file, contents);
    return file;
}

test("reads the compiler build out of the pin file", t => {
    let file = pinFile(t, `# a comment\n[toolchain]\nbuild = "swiftlang-1.2.3.4.5"\n`);

    assert.equal(pinnedSwift(file), "swiftlang-1.2.3.4.5");
});

test("refuses a pin file that names no compiler", t => {
    let file = pinFile(t, `[toolchain]\nversion = "6.4"\n`);

    assert.throws(() => pinnedSwift(file), /declares no \[toolchain\] build/);
});

// The exported constant is what every task compares the active compiler against, so a pin file
// the parser can read but that holds something else would go unnoticed.
test("the repository pins the Apple compiler the proposal requires", () => {
    let repository = fileURLToPath(new URL("../swift-toolchain.toml", import.meta.url));

    assert.equal(PINNED_SWIFT, "swiftlang-6.4.0.34.1");
    assert.equal(pinnedSwift(repository), PINNED_SWIFT);
});
