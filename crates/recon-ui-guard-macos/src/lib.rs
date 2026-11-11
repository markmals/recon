//! The continuous watch that makes `recon ui` safe to run while someone is working.
//!
//! A read must not take focus, move the pointer, change the active space, or raise a window, and
//! a before-and-after comparison cannot see a disturbance that was undone. So the guard watches
//! throughout the operation, attributes what it sees to the stage and the target the caller
//! declared, and classifies interference it could not rule out.
//!
//! Nothing here inspects an application. The guard observes the Mac through the same window
//! server and workspace interfaces anyone could use, and never calls back into the operation it
//! is watching.

#[cfg(target_os = "macos")]
mod focus;
#[cfg(target_os = "macos")]
mod private;
#[cfg(target_os = "macos")]
mod transient;

#[cfg(target_os = "macos")]
pub use focus::{Guard, Observation, Scope, Watch, guarded, guarded_interaction};
