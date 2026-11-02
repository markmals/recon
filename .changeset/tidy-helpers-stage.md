---
"recon": patch
---

`recon ui session prepare-host` now also installs Recon's research helper in `/Library/Application Support/Recon` through `sudo install`, which App Sandboxed applications such as TextEdit and Calculator can read. Run it again after upgrading Recon: research sessions use the installed helper only while it matches the running Recon's build, and otherwise use the packaged one. `--status` reports the installed helper as `stagedHelper`. `--revert` removes it, and the directory if nothing else is in it; removing only the helper reports `rebootRequired: false`. Applied and reverted results carry a new `stagedHelper` member naming the helper installed or removed.
