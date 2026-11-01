//! The contracts every Recon crate shares, and nothing a particular host or capability owns.
//!
//! [`action`] names an exported action and describes what an extension publishes about it,
//! [`error`] is the failure envelope the executable prints, [`process`] identifies one running
//! process, and [`schema`] turns a serialized payload's Rust type into the JSON Schema the
//! command registry publishes. Nothing here touches a platform interface, so the core builds
//! and tests anywhere.

pub mod action;
pub mod error;
pub mod process;
pub mod schema;
