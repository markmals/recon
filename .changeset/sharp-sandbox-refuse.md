---
"recon": patch
---

`recon ui session open` now refuses an App Sandboxed target, such as TextEdit or Calculator, with `INSPECTION_UNAVAILABLE` before loading anything into it. The error names the helper path the target's sandbox cannot read and the endpoint directory where it cannot listen. Previously the helper silently failed to start, the open timed out with `INSPECTION_TIMEOUT`, and later attempts returned `INSPECTION_BUSY` until the application quit.
