---
"recon": patch
---

`recon ui session open --profile research` now works for App Sandboxed applications such as TextEdit and Calculator. Previously the helper loaded but never answered, the open timed out with `INSPECTION_TIMEOUT`, and every retry returned `INSPECTION_BUSY` until the application quit. The sandbox still has to be able to read the helper. When it cannot, as with Homebrew's install directory, Recon now refuses with `INSPECTION_UNAVAILABLE` before loading anything, naming the helper path.
