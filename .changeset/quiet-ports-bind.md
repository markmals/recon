---
"recon": patch
---

`recon ui session open --profile research` no longer fails with `INSPECTION_UNAVAILABLE` "could not acquire the task control port (kern_return 5)" for targets such as TextEdit and Calculator. Recon now reads the selected process's identity without needing debugger rights, and only the entitled `recon-loader` takes control of the target, so the Homebrew-installed `recon` works without being re-signed.
