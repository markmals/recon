---
"recon": patch
---

Native inspection's in-process helper is now built from Swift, and `recon ui view` answers with the same observations as before. One difference is a correction: `coverage.complete` is now reported as `true` or `false`, as the published schema always said, where the previous helper reported `1` or `0`. Building Recon from source, including Homebrew's source build, now requires Apple's Swift 6.4 compiler build `swiftlang-6.4.0.34.1`, from Xcode 27.1 or the Command Line Tools; installing the bottle needs nothing new.
