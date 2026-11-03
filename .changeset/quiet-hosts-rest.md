---
"recon": patch
---

`recon ui session prepare-host` no longer fails with `UI_INTERFERENCE` when `sudo` asks for your password or Touch ID. Host preparation inspects no application, so it no longer runs under the watch that keeps `ui` commands from stealing focus, and `recon schema ui session prepare-host` no longer lists `UI_INTERFERENCE` or `INTERFACE_UNAVAILABLE`. Before this release the command reported that error after it had already applied its changes; `recon ui session prepare-host --status` shows what was applied. A `ui` command that fails because a privacy permission is missing now always reports `PERMISSION_DENIED`, even if macOS's request dialog came forward while it ran.
