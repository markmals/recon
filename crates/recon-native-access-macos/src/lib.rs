//! Access to one macOS process incarnation's own native objects, through the helper that runs
//! inside it.
//!
//! A session is an access relationship to one kernel-verified process incarnation, not to an
//! application, a window, or a saved observation. This crate owns what the host must get right
//! regardless of what is being inspected: that a descriptor only this user can read names one
//! incarnation, that a frame is bounded before it is believed, that a session capability never
//! leaves the descriptor, that an unacknowledged close stays uncertain until authenticated
//! reconciliation resolves it, and that the loaded helper matches the build the host expects.
//!
//! [`appkit`] is the first-party extension reached through that access. It adapts action
//! identities onto the helper's fixed wire operations; no walker, traversal, or field collection
//! lives on this side of the process boundary.

mod error;
mod session;
mod store;
mod transport;

pub mod identity;
pub mod request;

pub use error::{InspectionError, InspectionErrorCode, Stage};
pub use session::{
    CloseOutcome, OpenOutcome, OpenRequest, Reconciliation, close, close_until, native_request,
    open, open_until, reconcile,
};
pub use store::{SessionRecord, SessionState, SessionStore, UnresolvedExecution};
pub use transport::{
    Connection, LIFECYCLE_DEADLINE, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, NATIVE_DEADLINE, Op,
    PeerIdentity, current_incarnation, decode_frame_length, encode_frame,
};

#[cfg(target_os = "macos")]
mod access;
#[cfg(target_os = "macos")]
mod artifact;
#[cfg(target_os = "macos")]
mod host;
#[cfg(target_os = "macos")]
mod lifecycle;

#[cfg(all(test, target_os = "macos"))]
mod access_tests;

pub mod appkit;
#[cfg(target_os = "macos")]
pub mod inject;

#[cfg(target_os = "macos")]
pub use host::{failure, preflight, prepare_host};
#[cfg(target_os = "macos")]
pub use lifecycle::{close_session, open_session};
