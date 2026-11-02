---
"recon": patch
---

`recon ui session prepare-host` now also creates `/Library/Application Support/Recon` through `sudo`, owned by you, and copies Recon's research helper into it. App Sandboxed applications such as TextEdit and Calculator can read that location. Run it again after upgrading Recon: research sessions use the copied helper only while it is identical to the packaged one, and otherwise use the packaged helper. `--status` reports the copy as `stagedHelper`. `--revert` removes it, and the directory if nothing else is in it; removing only the helper reports `rebootRequired: false`. Applied and reverted results carry a new `stagedHelper` member naming the helper copied or removed.
