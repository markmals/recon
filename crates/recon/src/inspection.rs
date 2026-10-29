//! Host side of native inspection access: the owner-private session store, the framed local
//! transport, and the open/reconcile/close lifecycle that connects them.
//!
//! This module holds no native-object logic and no command behavior. It owns what the host must
//! get right regardless of what is being inspected: that a descriptor only this user can read
//! names one kernel-verified process incarnation, that a frame is bounded before it is believed,
//! that a session capability never leaves the descriptor, and that an unacknowledged close stays
//! uncertain until authenticated reconciliation resolves it.

mod error;
mod identity;
mod session;
mod store;
mod transport;

pub use error::{InspectionError, InspectionErrorCode, Stage};
pub use identity::{AccessProfile, AcquisitionMode, Capability, Incarnation, SessionId};
pub use session::{
    CloseOutcome, OpenOutcome, OpenRequest, Reconciliation, close, native_request, open, reconcile,
};
pub use store::{SessionRecord, SessionState, SessionStore, UnresolvedExecution};
pub use transport::{
    Connection, LIFECYCLE_DEADLINE, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, NATIVE_DEADLINE, Op,
    PeerIdentity, current_incarnation, decode_frame_length, encode_frame,
};
