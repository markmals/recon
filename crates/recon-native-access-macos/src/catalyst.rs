//! The host side of the first-party Catalyst interaction extension.
//!
//! The extension exports one action. It changes the target rather than observing it: it makes
//! the `UIWindow` hosted in a selected WindowServer window the target's key window, inside the
//! target, on its own main thread. It is reachable only through an open session, because a
//! Catalyst key chord runs the target's own code and that is something a person asks for.
//!
//! The wire operation an action is carried by is private bridge machinery: the action identity
//! is what the host routes on, and a name this extension does not export is a failure rather
//! than a substituted change.

use recon_core::action::{ActionId, ExtensionId};

/// The extension that owns Recon's Catalyst interaction.
pub const CATALYST: ExtensionId = ExtensionId::new("catalyst");

/// Makes the `UIWindow` hosted in the selected WindowServer window key inside the target.
pub const MAKE_WINDOW_KEY: ActionId = ActionId::new(CATALYST, "makeWindowKey");

#[cfg(target_os = "macos")]
mod interaction;

#[cfg(target_os = "macos")]
pub use interaction::{Catalyst, CatalystKeyWindow, make_window_key};
