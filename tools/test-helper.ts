// The helper package's own test suites: framing, tokens, response envelopes, and the
// authenticated access lifecycle driven over a real socket.
//
// They build the helper's sources in place rather than linking the shipped dylib, so the build
// identity they serve is a fixture rather than the linked one.
import { execFileSync } from "node:child_process";

if (process.platform === "darwin") {
    execFileSync("xcrun", ["swift", "test", "--package-path", "helper"], { stdio: "inherit" });
    console.log("Passed helper tests");
}
