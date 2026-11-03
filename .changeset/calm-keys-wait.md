---
"recon": patch
---

`recon ui` commands no longer fail with `UI_INTERFERENCE` ("keyboard, button or scroll events were posted into the session that the hardware did not generate") because you typed while they ran. macOS sometimes counts a real keystroke a moment later in the session than in the hardware, and Recon read that delay as input something else had posted. This most often hit `recon ui session prepare-host`, whose confirmation you type while it runs; when it happened, the preparation had already been applied. Input that the hardware never generated is still reported.
