//! One running process, as every capability refers to it.

use std::fmt;

use schemars::JsonSchema;
use serde::Serialize;

/// The process ID of one running instance of an application.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Pid(pub u32);

impl fmt::Display for Pid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
