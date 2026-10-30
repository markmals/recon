import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";

// Disabled at the owner's request: the pre-stop gate ran after every turn and, with a 25s cap
// against a run that takes far longer than that here — `mise run check` compiles the Rust
// workspace, builds the rustdoc and builds the record site — kept blocking stops. Run
// `mise run check` manually before claiming work is done; hk's `pre-push` hook and CI run it
// regardless. To restore the gate, see git history for the previous `session_stop` handler.
export default function checkBeforeStop(_pi: ExtensionAPI): void {}
