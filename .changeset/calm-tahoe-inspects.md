---
"recon": minor
---

Run native inspection on macOS 26 as well as macOS 27. `recon ui session preflight` and `open` no longer refuse a macOS 26 host, and the helper and loader declare macOS 26.0 as their minimum. The inspection commands were verified on macOS 26.6.2 in a virtual machine. Building from source on macOS 26 needs Xcode 27.2, which requires macOS 26.6 or later.
