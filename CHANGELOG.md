# recon

## 0.1.5

### Patch Changes

- [#88](https://github.com/markmals/recon/pull/88) [`db9cc2f`](https://github.com/markmals/recon/commit/db9cc2f5bbb2a456fc66520cc3424edd215cb03b) Thanks [@markmals](https://github.com/markmals)! - `recon ui session prepare-host` no longer fails with `UI_INTERFERENCE` when `sudo` asks for your password or Touch ID. Host preparation inspects no application, so it no longer runs under the watch that keeps `ui` commands from stealing focus, and `recon schema ui session prepare-host` no longer lists `UI_INTERFERENCE` or `INTERFACE_UNAVAILABLE`. Before this release the command reported that error after it had already applied its changes; `recon ui session prepare-host --status` shows what was applied. A `ui` command that fails because a privacy permission is missing now always reports `PERMISSION_DENIED`, even if macOS's request dialog came forward while it ran.

## 0.1.4

### Patch Changes

- [#86](https://github.com/markmals/recon/pull/86) [`5c556e8`](https://github.com/markmals/recon/commit/5c556e88b41ac064d4ef4b3cc0b7254fcadfb12a) Thanks [@markmals](https://github.com/markmals)! - `recon ui` commands no longer fail with `UI_INTERFERENCE` ("keyboard, button or scroll events were posted into the session that the hardware did not generate") because you typed while they ran. macOS sometimes counts a real keystroke a moment later in the session than in the hardware, and Recon read that delay as input something else had posted. This most often hit `recon ui session prepare-host`, whose confirmation you type while it runs; when it happened, the preparation had already been applied. Input that the hardware never generated is still reported.

## 0.1.3

### Patch Changes

- [#83](https://github.com/markmals/recon/pull/83) [`bce2b56`](https://github.com/markmals/recon/commit/bce2b56d91b384905102d11382dbafde069b97ac) Thanks [@markmals](https://github.com/markmals)! - `recon ui session open --profile research` no longer fails with `INSPECTION_UNAVAILABLE` "could not acquire the task control port (kern_return 5)" for targets such as TextEdit and Calculator. Recon now reads the selected process's identity without needing debugger rights, and only the entitled `recon-loader` takes control of the target, so the Homebrew-installed `recon` works without being re-signed.

- [#82](https://github.com/markmals/recon/pull/82) [`da2074d`](https://github.com/markmals/recon/commit/da2074dd77cbfba89a4c65b0aa545247b9ae6b73) Thanks [@markmals](https://github.com/markmals)! - Native inspection's in-process helper is now built from Swift, and `recon ui view` answers with the same observations as before. One difference is a correction: `coverage.complete` is now reported as `true` or `false`, as the published schema always said, where the previous helper reported `1` or `0`. Building Recon from source, including Homebrew's source build, now requires Apple's Swift 6.4 compiler build `swiftlang-6.4.0.34.1`, from Xcode 27.1 or the Command Line Tools; installing the bottle needs nothing new.

- [#83](https://github.com/markmals/recon/pull/83) [`2ba8caf`](https://github.com/markmals/recon/commit/2ba8cafd61fbfacb9236995ea13346e1752613f5) Thanks [@markmals](https://github.com/markmals)! - `recon ui session open --profile research` now works for App Sandboxed applications such as TextEdit and Calculator. Previously the helper loaded but never answered, the open timed out with `INSPECTION_TIMEOUT`, and every retry returned `INSPECTION_BUSY` until the application quit. The sandbox still has to be able to read the helper. When it cannot, as with Homebrew's install directory, Recon now refuses with `INSPECTION_UNAVAILABLE` before loading anything, naming the helper path.

- [#83](https://github.com/markmals/recon/pull/83) [`79d9ed1`](https://github.com/markmals/recon/commit/79d9ed12107fb54ead1eac752e114f8ef0b6712a) Thanks [@markmals](https://github.com/markmals)! - `recon ui session prepare-host` now also creates `/Library/Application Support/Recon` through `sudo`, owned by you, and copies Recon's research helper into it. App Sandboxed applications such as TextEdit and Calculator can read that location. Run it again after upgrading Recon: research sessions use the copied helper only while it is identical to the packaged one, and otherwise use the packaged helper. `--status` reports the copy as `stagedHelper`. `--revert` removes it, and the directory if nothing else is in it; removing only the helper reports `rebootRequired: false`. Applied and reverted results carry a new `stagedHelper` member naming the helper copied or removed.
