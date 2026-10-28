import type { ExtensionAPI } from "@oh-my-pi/pi-coding-agent";

// Disabled at the owner's request: the pre-stop `vp run check` gate ran after every turn and, with a
// 25s cap against a ~36s cached run, kept blocking stops. Run `vp run check` manually when needed.
// To restore the gate, see git history for the previous `session_stop` handler.
export default function checkBeforeStop(_pi: ExtensionAPI): void {}
