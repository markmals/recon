//! The framed local transport: one length-prefixed JSON object in each direction over an
//! owner-only Unix-domain endpoint.
//!
//! Framing limits are enforced against the length the peer supplies, before any buffer is sized
//! from it, so neither endpoint can make the other allocate on request. Every connection is
//! checked against the kernel's report of who is on the far end before a request is written.

use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};

use super::error::{InspectionError, InspectionErrorCode, Stage};
use super::identity::Incarnation;

/// The largest request frame, matching proposal 0005's 64 KiB request cap.
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
/// The largest response frame, matching the 8 MiB response cap.
pub const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Covers queueing, target execution, transport, and delivery of one native read.
pub const NATIVE_DEADLINE: Duration = Duration::from_secs(2);
/// Covers an open, close, or reconciliation round trip.
pub const LIFECYCLE_DEADLINE: Duration = Duration::from_secs(10);

/// The operations this transport can carry. Native object semantics live in the helper; the
/// transport only distinguishes lifecycle work from a native read, because they have different
/// deadlines and different consequences when they time out.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Op {
    Open,
    Reconcile,
    Close,
    Windows,
    Tree,
    Node,
    Find,
    Fonts,
}

impl Op {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Reconcile => "reconcile",
            Self::Close => "close",
            Self::Windows => "windows",
            Self::Tree => "tree",
            Self::Node => "node",
            Self::Find => "find",
            Self::Fonts => "fonts",
        }
    }

    /// Whether the operation negotiates access rather than reading native objects.
    pub fn is_lifecycle(self) -> bool {
        matches!(self, Self::Open | Self::Reconcile | Self::Close)
    }

    pub fn deadline(self) -> Duration {
        if self.is_lifecycle() { LIFECYCLE_DEADLINE } else { NATIVE_DEADLINE }
    }
}

/// Who the kernel says is on the far end of a connected endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PeerIdentity {
    pub uid: u32,
    pub incarnation: Incarnation,
}

/// A connected, peer-verified inspection channel.
#[derive(Debug)]
pub struct Connection {
    stream: UnixStream,
    peer: PeerIdentity,
}

impl Connection {
    /// Connect to a helper endpoint and confirm the owner is on the far end.
    ///
    /// The endpoint's own permissions are checked first: a socket that is not an owner-only
    /// regular socket file is refused rather than connected to, because a predictable or
    /// world-writable endpoint is exactly the prior art's mistake.
    pub fn connect(socket_path: &Path) -> Result<Self, InspectionError> {
        verify_endpoint(socket_path)?;
        let stream = UnixStream::connect(socket_path).map_err(|error| {
            InspectionError::from_io(
                InspectionErrorCode::Unavailable,
                Stage::Connect,
                &format!("the helper endpoint at {} could not be reached", socket_path.display()),
                &error,
            )
        })?;
        let peer = peer_identity(&stream)?;
        let owner = current_uid();
        if let Some(owner) = owner
            && peer.uid != owner
        {
            return Err(InspectionError::new(
                InspectionErrorCode::Unavailable,
                Stage::Peer,
                format!("the helper endpoint is held by uid {}, not by uid {owner}", peer.uid),
            ));
        }
        Ok(Self { stream, peer })
    }

    pub fn peer(&self) -> PeerIdentity {
        self.peer
    }

    /// Refuse a peer that is not the recorded incarnation. A pid that has been reused names a
    /// different process, and its generation says so.
    pub fn require_incarnation(&self, expected: Incarnation) -> Result<(), InspectionError> {
        if self.peer.incarnation == expected {
            Ok(())
        } else {
            Err(InspectionError::new(
                InspectionErrorCode::StaleSession,
                Stage::Peer,
                format!(
                    "the endpoint is now held by {} but the session is bound to {expected}",
                    self.peer.incarnation
                ),
            ))
        }
    }

    /// Send one request and read its response, within the operation's deadline.
    ///
    /// `body` must be a JSON object; `op` is written into it. A helper failure envelope becomes an
    /// `Err`, so a caller never has to inspect `ok` itself.
    pub fn request(&mut self, op: Op, body: Value) -> Result<Map<String, Value>, InspectionError> {
        self.request_until(op, body, Instant::now() + op.deadline())
    }

    /// Use an open operation's existing absolute deadline instead of restarting its budget.
    pub fn request_until(
        &mut self,
        op: Op,
        body: Value,
        deadline: Instant,
    ) -> Result<Map<String, Value>, InspectionError> {
        let deadline = deadline.min(Instant::now() + op.deadline());
        let Value::Object(mut request) = body else {
            return Err(InspectionError::new(
                InspectionErrorCode::InvalidArgument,
                Stage::Send,
                "a request body must be a JSON object",
            ));
        };
        request.insert("op".to_owned(), Value::String(op.as_wire().to_owned()));

        // poll bounds readiness, and nonblocking I/O prevents a ready socket from then waiting
        // past the deadline while writing a frame larger than its remaining send buffer.
        self.stream.set_nonblocking(true).map_err(|error| {
            InspectionError::from_io(
                InspectionErrorCode::Failed,
                Stage::Send,
                "the helper channel could not be made nonblocking",
                &error,
            )
        })?;
        let frame = encode_frame(&Value::Object(request), MAX_REQUEST_BYTES)?;
        self.write_until(op, &frame, deadline)?;
        let payload = self.read_frame(op, deadline)?;
        let response = decode_response(op, &payload)?;
        if op != Op::Open && remaining(deadline).is_none() {
            let error = InspectionError::new(
                InspectionErrorCode::Timeout,
                Stage::Receive,
                format!("the {} deadline expired before the response was delivered", op.as_wire()),
            );
            return Err(if op.is_lifecycle() {
                error
            } else {
                error.target_execution_may_continue()
            });
        }
        Ok(response)
    }

    fn wait_ready(&self, op: Op, stage: Stage, deadline: Instant) -> Result<(), InspectionError> {
        let events = if stage == Stage::Send { libc::POLLOUT } else { libc::POLLIN };
        loop {
            let Some(left) = remaining(deadline) else {
                let error = InspectionError::new(
                    InspectionErrorCode::Timeout,
                    stage,
                    format!("the {} deadline expired before the {stage} stage", op.as_wire()),
                );
                return Err(if op.is_lifecycle() {
                    error
                } else {
                    error.target_execution_may_continue()
                });
            };
            let timeout_ms = left.as_millis().saturating_add(1).min(i32::MAX as u128) as i32;
            let mut descriptor = libc::pollfd { fd: self.stream.as_raw_fd(), events, revents: 0 };
            // SAFETY: poll receives one writable pollfd whose file descriptor is owned by self.
            let ready = unsafe { libc::poll(&raw mut descriptor, 1, timeout_ms) };
            if ready > 0 {
                if descriptor.revents & libc::POLLNVAL != 0 {
                    return Err(InspectionError::new(
                        InspectionErrorCode::Failed,
                        stage,
                        "the helper connection became invalid while waiting for I/O",
                    ));
                }
                return Ok(());
            }
            if ready == 0 {
                continue;
            }
            let error = std::io::Error::last_os_error();
            if error.kind() != std::io::ErrorKind::Interrupted {
                return Err(InspectionError::from_io(
                    InspectionErrorCode::Failed,
                    stage,
                    "the helper connection could not be polled",
                    &error,
                ));
            }
        }
    }

    fn write_until(
        &mut self,
        op: Op,
        frame: &[u8],
        deadline: Instant,
    ) -> Result<(), InspectionError> {
        let mut sent = 0;
        while sent < frame.len() {
            self.wait_ready(op, Stage::Send, deadline)?;
            match self.stream.write(&frame[sent..]) {
                Ok(0) => {
                    return Err(InspectionError::new(
                        InspectionErrorCode::Failed,
                        Stage::Send,
                        "the helper connection stopped accepting request bytes",
                    ));
                }
                Ok(written) => sent += written,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(error) => {
                    return Err(io_failure(
                        op,
                        Stage::Send,
                        "the request could not be written",
                        &error,
                    ));
                }
            }
        }
        Ok(())
    }

    fn read_exact_until(
        &mut self,
        op: Op,
        buffer: &mut [u8],
        deadline: Instant,
        context: &str,
    ) -> Result<(), InspectionError> {
        let mut received = 0;
        while received < buffer.len() {
            self.wait_ready(op, Stage::Receive, deadline)?;
            match self.stream.read(&mut buffer[received..]) {
                Ok(0) => {
                    return Err(InspectionError::new(
                        InspectionErrorCode::Failed,
                        Stage::Receive,
                        format!("{context}: the helper closed the connection early"),
                    ));
                }
                Ok(read) => received += read,
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(error) => return Err(io_failure(op, Stage::Receive, context, &error)),
            }
        }
        Ok(())
    }

    fn read_frame(&mut self, op: Op, deadline: Instant) -> Result<Vec<u8>, InspectionError> {
        let mut header = [0_u8; 4];
        self.read_exact_until(op, &mut header, deadline, "the response length could not be read")?;
        let length = decode_frame_length(header, MAX_RESPONSE_BYTES)?;

        let mut payload = vec![0_u8; length];
        self.read_exact_until(op, &mut payload, deadline, "the response body could not be read")?;
        Ok(payload)
    }
}

/// Validate a frame length supplied by the peer against `cap`.
///
/// This runs on the four header bytes alone: the caller sizes its buffer from the returned length,
/// so an advertised 4 GiB response is refused without ever being allocated.
pub fn decode_frame_length(header: [u8; 4], cap: usize) -> Result<usize, InspectionError> {
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 {
        return Err(InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Receive,
            "the peer announced an empty frame",
        ));
    }
    if length > cap {
        return Err(InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Receive,
            format!("the peer announced a {length}-byte frame, over the {cap}-byte cap"),
        ));
    }
    Ok(length)
}

/// Serialize `value` into a length-prefixed frame, refusing one that exceeds `cap`.
///
/// The prefix is reserved before serializing so the payload is never copied a second time.
pub fn encode_frame(value: &Value, cap: usize) -> Result<Vec<u8>, InspectionError> {
    let mut frame = vec![0_u8; 4];
    serde_json::to_writer(&mut frame, value).map_err(|error| {
        InspectionError::new(
            InspectionErrorCode::InvalidArgument,
            Stage::Send,
            format!("the request could not be serialized: {error}"),
        )
    })?;
    let length = frame.len() - 4;
    if length > cap {
        return Err(InspectionError::new(
            InspectionErrorCode::InvalidArgument,
            Stage::Send,
            format!("a {length}-byte request exceeds the {cap}-byte cap"),
        ));
    }
    let header = u32::try_from(length).map_err(|_| {
        InspectionError::new(
            InspectionErrorCode::InvalidArgument,
            Stage::Send,
            "the request length does not fit the frame header",
        )
    })?;
    frame[..4].copy_from_slice(&header.to_be_bytes());
    Ok(frame)
}

/// Decode one response frame into its object body, turning a helper failure envelope into an error.
fn decode_response(op: Op, payload: &[u8]) -> Result<Map<String, Value>, InspectionError> {
    let value: Value = serde_json::from_slice(payload).map_err(|error| {
        InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Receive,
            format!("the {} response was not valid JSON: {error}", op.as_wire()),
        )
    })?;
    let Value::Object(response) = value else {
        return Err(InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Receive,
            format!("the {} response was not a JSON object", op.as_wire()),
        ));
    };
    match response.get("ok").and_then(Value::as_bool) {
        Some(true) => Ok(response),
        Some(false) => {
            let error = response.get("error").cloned().unwrap_or(Value::Null);
            Err(InspectionError::from_wire(Stage::Receive, &error))
        }
        None => Err(InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Receive,
            format!("the {} response carried no `ok` field", op.as_wire()),
        )),
    }
}

fn remaining(deadline: Instant) -> Option<Duration> {
    deadline.checked_duration_since(Instant::now()).filter(|left| !left.is_zero())
}

/// An expired channel deadline is a timeout; a native read that times out may still be running on
/// the target's main thread, which Recon never cancels.
fn io_failure(op: Op, stage: Stage, context: &str, error: &std::io::Error) -> InspectionError {
    let timed_out =
        matches!(error.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut);
    if timed_out {
        let failure = InspectionError::new(
            InspectionErrorCode::Timeout,
            stage,
            format!("{context}: the {} deadline expired", op.as_wire()),
        );
        if op.is_lifecycle() { failure } else { failure.target_execution_may_continue() }
    } else {
        InspectionError::from_io(InspectionErrorCode::Failed, stage, context, error)
    }
}

/// Refuse an endpoint that is not an owner-only socket. The helper creates it inside a directory
/// the host made 0700; both halves are checked because either one alone can be loosened.
fn verify_endpoint(socket_path: &Path) -> Result<(), InspectionError> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};

    let metadata = std::fs::symlink_metadata(socket_path).map_err(|error| {
        InspectionError::from_io(
            InspectionErrorCode::Unavailable,
            Stage::Connect,
            &format!("the helper endpoint at {} is unreadable", socket_path.display()),
            &error,
        )
    })?;
    if !metadata.file_type().is_socket() {
        return Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Connect,
            format!("{} is not a socket", socket_path.display()),
        ));
    }
    let mode = metadata.permissions().mode() & 0o777;
    if mode != 0o600 {
        return Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Connect,
            format!(
                "the helper endpoint at {} is mode {mode:04o}, not 0600",
                socket_path.display()
            ),
        ));
    }
    if let Some(owner) = current_uid()
        && metadata.uid() != owner
    {
        return Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Connect,
            format!(
                "the helper endpoint at {} belongs to uid {}, not to uid {owner}",
                socket_path.display(),
                metadata.uid()
            ),
        ));
    }
    Ok(())
}

/// The effective uid, where the platform's uid API is linked. `None` means ownership cannot be
/// compared here; this feature only ships on macOS, where it always can.
#[cfg(target_os = "macos")]
pub(super) fn current_uid() -> Option<u32> {
    // SAFETY: `geteuid` reads this process's credentials and cannot fail.
    Some(unsafe { libc::geteuid() })
}

#[cfg(not(target_os = "macos"))]
pub(super) fn current_uid() -> Option<u32> {
    None
}

/// This process's own kernel-verified identity, which every request carries so the helper can
/// check the client it is answering against its own `LOCAL_PEERTOKEN` view of the connection.
#[cfg(target_os = "macos")]
pub fn current_incarnation() -> Result<Incarnation, InspectionError> {
    /// `TASK_AUDIT_TOKEN` and its length in `natural_t` words, from `<mach/task_info.h>`.
    const TASK_AUDIT_TOKEN: libc::c_int = 15;
    const TASK_AUDIT_TOKEN_COUNT: u32 = 8;

    unsafe extern "C" {
        /// An immutable global libSystem initializes before `main`, so reading it is a plain load.
        static mach_task_self_: libc::mach_port_t;

        /// Declared as in `<mach/task.h>`, matching the loader's own declaration.
        fn task_info(
            task: libc::mach_port_t,
            flavor: libc::c_int,
            info: *mut libc::c_int,
            count: *mut libc::mach_msg_type_number_t,
        ) -> libc::kern_return_t;
    }

    let mut token = [0_u32; 8];
    let mut count = TASK_AUDIT_TOKEN_COUNT;
    // SAFETY: `token` is eight words long, which is what `TASK_AUDIT_TOKEN` writes, and `count`
    // says so; `mach_task_self_` is this process's own task port.
    let status = unsafe {
        task_info(mach_task_self_, TASK_AUDIT_TOKEN, token.as_mut_ptr().cast(), &raw mut count)
    };
    if status != 0 {
        return Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Peer,
            format!("this process's own audit token could not be read: Mach status {status}"),
        ));
    }
    Ok(Incarnation { pid: token[5] as i32, pidversion: token[7] })
}

#[cfg(not(target_os = "macos"))]
pub fn current_incarnation() -> Result<Incarnation, InspectionError> {
    Err(InspectionError::new(
        InspectionErrorCode::Unavailable,
        Stage::Peer,
        "kernel-backed process identity is only available on macOS",
    ))
}

/// Read the peer's audit token: its effective uid, pid, and the pid generation that makes the pid
/// an identity rather than a number.
#[cfg(target_os = "macos")]
fn peer_identity(stream: &UnixStream) -> Result<PeerIdentity, InspectionError> {
    use std::os::fd::AsRawFd;

    /// `SOL_LOCAL` and `LOCAL_PEERTOKEN` from `<sys/un.h>`.
    const SOL_LOCAL: libc::c_int = 0;
    const LOCAL_PEERTOKEN: libc::c_int = 0x006;

    /// `audit_token_t`: `val[1]` is the effective uid, `val[5]` the pid, `val[7]` the pid
    /// generation, per `<bsm/libbsm.h>`'s accessors.
    #[repr(C)]
    struct AuditToken {
        val: [u32; 8],
    }

    let mut token = AuditToken { val: [0; 8] };
    let mut length = size_of::<AuditToken>() as libc::socklen_t;
    // SAFETY: `token` is a live, correctly sized `audit_token_t` and `length` describes it; the fd
    // is owned by `stream` for the duration of the call.
    let status = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            SOL_LOCAL,
            LOCAL_PEERTOKEN,
            (&raw mut token).cast(),
            &raw mut length,
        )
    };
    if status != 0 || length as usize != size_of::<AuditToken>() {
        return Err(InspectionError::from_io(
            InspectionErrorCode::Unavailable,
            Stage::Peer,
            "the peer's audit token could not be read",
            &std::io::Error::last_os_error(),
        ));
    }
    Ok(PeerIdentity {
        uid: token.val[1],
        incarnation: Incarnation { pid: token.val[5] as i32, pidversion: token.val[7] },
    })
}

#[cfg(not(target_os = "macos"))]
fn peer_identity(_stream: &UnixStream) -> Result<PeerIdentity, InspectionError> {
    Err(InspectionError::new(
        InspectionErrorCode::Unavailable,
        Stage::Peer,
        "kernel-backed peer identity is only available on macOS",
    ))
}
