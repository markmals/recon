//! The identities a session is made of: the process incarnation it is bound to, its random
//! identifier, and its secret capability.

use std::fmt;
use std::fs::File;
use std::io::Read;

use super::error::{InspectionError, InspectionErrorCode, Stage};

/// Bytes of randomness behind a session identifier. The identifier is only a name; the capability
/// carries the authority.
const SESSION_ID_BYTES: usize = 16;
/// Bytes of randomness behind a session capability.
const CAPABILITY_BYTES: usize = 32;

/// A kernel-verified process identity in its storable form: the loader's binding, written down.
///
/// A pid alone is not an identity because the kernel reuses pids; `pidversion` is the generation
/// that distinguishes one incarnation from a later process holding the same number.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Incarnation {
    pub pid: i32,
    pub pidversion: u32,
}

impl Incarnation {
    pub fn new(pid: i32, pidversion: u32) -> Self {
        Self { pid, pidversion }
    }

    /// The stable, filesystem-safe spelling used for per-incarnation directories and guard files.
    pub fn slug(self) -> String {
        format!("{}-{}", self.pid, self.pidversion)
    }
}

impl fmt::Display for Incarnation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "pid {} generation {}", self.pid, self.pidversion)
    }
}

#[cfg(target_os = "macos")]
impl From<crate::macos::inject::Incarnation> for Incarnation {
    fn from(bound: crate::macos::inject::Incarnation) -> Self {
        Self { pid: bound.pid, pidversion: bound.pidversion }
    }
}

#[cfg(target_os = "macos")]
impl From<Incarnation> for crate::macos::inject::Incarnation {
    fn from(recorded: Incarnation) -> Self {
        Self { pid: recorded.pid, pidversion: recorded.pidversion }
    }
}

/// How a session's access to the target was originally acquired, or how a later request asks to
/// reach it. The two are recorded separately: connecting with `existing` never rewrites a session
/// that `launch` established.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcquisitionMode {
    Existing,
    Launch,
    Attach,
}

impl AcquisitionMode {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Existing => "existing",
            Self::Launch => "launch",
            Self::Attach => "attach",
        }
    }

    pub fn from_wire(mode: &str) -> Option<Self> {
        match mode {
            "existing" => Some(Self::Existing),
            "launch" => Some(Self::Launch),
            "attach" => Some(Self::Attach),
            _ => None,
        }
    }
}

impl fmt::Display for AcquisitionMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_wire())
    }
}

/// The access profile a session runs under. A profile conflict fails rather than upgrading or
/// downgrading an established session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessProfile {
    Standard,
    Research,
}

impl AccessProfile {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Research => "research",
        }
    }

    pub fn from_wire(profile: &str) -> Option<Self> {
        match profile {
            "standard" => Some(Self::Standard),
            "research" => Some(Self::Research),
            _ => None,
        }
    }
}

impl fmt::Display for AccessProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_wire())
    }
}

/// A session's random identifier. It names a session in commands and on disk, so it is validated
/// as lowercase hex of a fixed length before it is ever used as a path component.
#[derive(Clone, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct SessionId(String);

impl SessionId {
    pub fn generate() -> Result<Self, InspectionError> {
        Ok(Self(random_hex(SESSION_ID_BYTES)?))
    }

    /// Accept an identifier from a command line or a helper response. Anything but the exact
    /// lowercase-hex shape is refused, which is also what keeps a helper-supplied identifier from
    /// escaping the descriptor directory.
    pub fn parse(text: &str) -> Result<Self, InspectionError> {
        if text.len() == SESSION_ID_BYTES * 2 && text.bytes().all(is_lowercase_hex) {
            Ok(Self(text.to_owned()))
        } else {
            Err(InspectionError::new(
                InspectionErrorCode::InvalidArgument,
                Stage::Store,
                format!("a session id must be {} lowercase hex digits", SESSION_ID_BYTES * 2),
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A session's secret capability.
///
/// Its `Debug` and `Display` are redacted and it has no serialization: the secret leaves only
/// through [`Capability::expose`], which the store calls when writing an owner-only descriptor and
/// the transport calls when authenticating a request.
#[derive(Clone, Eq, PartialEq)]
pub struct Capability(String);

impl Capability {
    pub fn generate() -> Result<Self, InspectionError> {
        Ok(Self(random_hex(CAPABILITY_BYTES)?))
    }

    pub fn parse(text: &str) -> Result<Self, InspectionError> {
        if text.len() == CAPABILITY_BYTES * 2 && text.bytes().all(is_lowercase_hex) {
            Ok(Self(text.to_owned()))
        } else {
            Err(InspectionError::new(
                InspectionErrorCode::Failed,
                Stage::Handshake,
                format!(
                    "a session capability must be {} lowercase hex digits",
                    CAPABILITY_BYTES * 2
                ),
            ))
        }
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Capability(<redacted>)")
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}

fn is_lowercase_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

/// Draw `bytes` of kernel randomness and hex-encode it. `/dev/urandom` is the dependency-free way
/// to reach the same CSPRNG `arc4random` uses.
fn random_hex(bytes: usize) -> Result<String, InspectionError> {
    let mut raw = vec![0_u8; bytes];
    let mut source = File::open("/dev/urandom").map_err(|error| {
        InspectionError::from_io(
            InspectionErrorCode::Unavailable,
            Stage::Store,
            "/dev/urandom could not be opened",
            &error,
        )
    })?;
    source.read_exact(&mut raw).map_err(|error| {
        InspectionError::from_io(
            InspectionErrorCode::Unavailable,
            Stage::Store,
            "/dev/urandom could not be read",
            &error,
        )
    })?;

    let mut hex = String::with_capacity(bytes * 2);
    for byte in raw {
        hex.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        hex.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    Ok(hex)
}
