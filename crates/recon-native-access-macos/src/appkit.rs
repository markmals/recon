//! The host side of the first-party AppKit extension.
//!
//! The extension exports five actions. Each one names a read of the target's own AppKit objects,
//! performed inside the target by the resident helper, and each is reachable only through the
//! session whose access it belongs to. The wire operation an action is carried by is private
//! bridge machinery: the action identity is what the host routes on, and a name this extension
//! does not export is a failure rather than a substituted observation.

use recon_core::action::{ActionId, ExtensionId};

/// The extension that owns Recon's AppKit reads.
pub const APPKIT: ExtensionId = ExtensionId::new("appkit");

/// The native window inventory of the target, as AppKit itself reports it.
pub const WINDOWS: ActionId = ActionId::new(APPKIT, "windows");
/// A bounded walk of the target's view or layer containment.
pub const TREE: ActionId = ActionId::new(APPKIT, "tree");
/// One named live object and the field groups asked for.
pub const NODE: ActionId = ActionId::new(APPKIT, "node");
/// The objects in scope that match every supplied filter.
pub const FIND: ActionId = ActionId::new(APPKIT, "find");
/// The fonts the objects in scope draw with.
pub const FONTS: ActionId = ActionId::new(APPKIT, "fonts");

#[cfg(target_os = "macos")]
mod reads;

#[cfg(target_os = "macos")]
pub use reads::{AppKit, ViewQuery, session_record, validate_session};
