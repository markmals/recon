//! The session lifecycle: establishing access, resolving an uncertain close, and carrying a
//! native request under the process-wide read guard.
//!
//! The rules this file exists to keep: an unacknowledged close becomes `close-unconfirmed` rather
//! than an invented success; a session that may still be live is never replaced; and reconciliation
//! resolves uncertainty only through authenticated bootstrap state or a confirmed process exit.

use std::path::Path;

use serde_json::{Map, Value};

use super::error::{InspectionError, InspectionErrorCode, Stage};
use super::identity::{AccessProfile, AcquisitionMode, Capability, Incarnation, SessionId};
use super::store::{SessionRecord, SessionState, SessionStore, now_unix};
use super::transport::{Connection, Op, current_incarnation, current_uid};

/// What a caller asks for when opening a session. Identity comes from the loader's verified
/// binding; nothing here resolves a bundle or a pid.
#[derive(Clone, Copy, Debug)]
pub struct OpenRequest<'a> {
    pub socket_path: &'a Path,
    pub bundle_path: &'a Path,
    pub bundle_id: &'a str,
    pub incarnation: Incarnation,
    pub mode: AcquisitionMode,
    pub profile: AccessProfile,
    pub helper_build: &'a str,
    pub protocol: u32,
}

/// A session that is now recorded, plus the helper's own account of the open.
#[derive(Clone, Debug)]
pub struct OpenOutcome {
    pub record: SessionRecord,
    /// Whether the helper answered with the session it already had, rather than a new one.
    pub reused: bool,
    /// The helper's response, for the caller to project into a command result. It never contains
    /// the capability once this outcome is built; the capability lives only in `record`.
    pub response: Map<String, Value>,
}

/// What authenticated bootstrap state says about a session whose fate the host is unsure of.
#[derive(Debug)]
pub enum Reconciliation {
    /// The helper still holds this session. An uncertain close is not resolved by this: the caller
    /// must close it again.
    Active,
    /// The helper confirms the session is revoked.
    Revoked,
    /// The bound incarnation is gone, which invalidates bootstrap and session access alike.
    ProcessExited,
    /// Bootstrap could not be reached or refused to answer. The uncertainty is preserved.
    Unreachable(InspectionError),
}

/// How a close ended. Each variant is a distinguishable result, including the one that admits the
/// outcome is unknown.
#[derive(Debug)]
pub enum CloseOutcome {
    /// The helper acknowledged revocation on this call.
    Acknowledged {
        /// Whether the helper still had the session active when it revoked it.
        was_active: bool,
    },
    /// The host already knew this session was closed, so no helper work was needed.
    AlreadyClosed,
    /// An earlier uncertain close was resolved by reconciliation rather than by this call.
    ReconciledClosed { process_exited: bool },
    /// Revocation was requested and never confirmed. The descriptor is retained in
    /// `close-unconfirmed`; the target may still hold the session.
    Unconfirmed(InspectionError),
}

/// Establish or reuse one session for a verified process incarnation.
pub fn open(
    store: &SessionStore,
    request: &OpenRequest<'_>,
) -> Result<OpenOutcome, InspectionError> {
    let live = store.live_record_for(request.incarnation)?;
    let mut reusable = None;

    if let Some(record) = &live {
        match record.state {
            // Revocation of the previous session is uncertain: resolve it before considering a
            // replacement, and refuse one while it stays uncertain.
            SessionState::CloseUnconfirmed => match reconcile_record(store, record)? {
                Reconciliation::Active => {
                    return Err(InspectionError::new(
                        InspectionErrorCode::Busy,
                        Stage::Handshake,
                        format!(
                            "session {} is still active for {} after an unconfirmed close; close it again before opening another",
                            record.session_id, record.incarnation
                        ),
                    ));
                }
                Reconciliation::Revoked | Reconciliation::ProcessExited => {}
                Reconciliation::Unreachable(cause) => {
                    return Err(InspectionError::new(
                        InspectionErrorCode::Unavailable,
                        Stage::Reconcile,
                        format!(
                            "whether session {} was revoked for {} is still unknown ({cause}); Recon will not open a replacement session",
                            record.session_id, record.incarnation
                        ),
                    ));
                }
            },
            SessionState::Active => {
                if record.helper_build != request.helper_build
                    || record.protocol != request.protocol
                {
                    return Err(InspectionError::new(
                        InspectionErrorCode::ProtocolMismatch,
                        Stage::Handshake,
                        format!(
                            "session {} runs helper build {} on protocol {}, not build {} on protocol {}",
                            record.session_id,
                            record.helper_build,
                            record.protocol,
                            request.helper_build,
                            request.protocol
                        ),
                    ));
                }
                if record.profile != request.profile {
                    return Err(InspectionError::new(
                        InspectionErrorCode::Busy,
                        Stage::Handshake,
                        format!(
                            "session {} holds the {} profile for {}; close it before opening a {} session",
                            record.session_id, record.profile, record.incarnation, request.profile
                        ),
                    ));
                }
                reusable = Some(record.clone());
            }
            SessionState::Closed => {}
        }
    }

    let mut connection = Connection::connect(request.socket_path)?;
    connection.require_incarnation(request.incarnation)?;

    let mut body = envelope(request.protocol, request.helper_build)?;
    body.insert("expectedPid".to_owned(), Value::from(request.incarnation.pid));
    body.insert("expectedPidversion".to_owned(), Value::from(request.incarnation.pidversion));
    if let Some(owner) = current_uid() {
        body.insert("expectedUid".to_owned(), Value::from(owner));
    }
    body.insert("mode".to_owned(), Value::from(request.mode.as_wire()));
    body.insert("profile".to_owned(), Value::from(request.profile.as_wire()));
    // Reuse is authenticated by the capability the host already holds, so a bare connection to the
    // endpoint cannot collect a live session's capability.
    if let Some(prior) = &reusable {
        body.insert("sessionId".to_owned(), Value::from(prior.session_id.as_str()));
        body.insert("capability".to_owned(), Value::from(prior.capability.expose()));
    }

    let mut response = connection.request(Op::Open, Value::Object(body))?;
    require_compatible(&response, request.protocol, request.helper_build)?;
    require_incarnation(&response, request.incarnation)?;

    let session_id = SessionId::parse(string_field(&response, "sessionId")?)?;
    let reused = response.get("reused").and_then(Value::as_bool).unwrap_or(false);
    let now = now_unix();

    let record = match (&reusable, reused) {
        (Some(prior), true) => {
            if prior.session_id != session_id {
                return Err(InspectionError::new(
                    InspectionErrorCode::Failed,
                    Stage::Handshake,
                    format!(
                        "the helper reported reusing session {session_id} while {} is recorded for {}",
                        prior.session_id, prior.incarnation
                    ),
                ));
            }
            let capability = match response.get("capability").and_then(Value::as_str) {
                Some(reported) => Capability::parse(reported)?,
                None => prior.capability.clone(),
            };
            // The acquisition history is the host's: a later `existing` request never rewrites
            // the mode that established the session.
            SessionRecord {
                capability,
                requested_mode: request.mode,
                updated_at: now,
                ..prior.clone()
            }
        }
        (Some(prior), false) => {
            return Err(InspectionError::new(
                InspectionErrorCode::Failed,
                Stage::Handshake,
                format!(
                    "the helper opened session {session_id} while session {} is recorded active for {}",
                    prior.session_id, prior.incarnation
                ),
            ));
        }
        (None, _) => {
            let capability = Capability::parse(string_field(&response, "capability")?)?;
            let established = response
                .get("establishedMode")
                .and_then(Value::as_str)
                .and_then(AcquisitionMode::from_wire)
                .unwrap_or(request.mode);
            SessionRecord {
                session_id,
                capability,
                bundle_path: request.bundle_path.to_path_buf(),
                bundle_id: request.bundle_id.to_owned(),
                incarnation: request.incarnation,
                established_mode: established,
                requested_mode: request.mode,
                profile: request.profile,
                helper_build: request.helper_build.to_owned(),
                protocol: request.protocol,
                socket_path: request.socket_path.to_path_buf(),
                state: SessionState::Active,
                opened_at: now,
                updated_at: now,
            }
        }
    };

    let persisted = if reusable.is_some() { store.save(&record) } else { store.insert(&record) };
    if let Err(error) = persisted {
        // The helper acknowledged a new session but the host could not record the means to revoke
        // it. Rather than leave an orphaned active session the host can neither reuse nor close,
        // revoke it on the helper before surfacing the failure. A newly established session is
        // revoked; a reused one keeps the still-recorded prior session untouched.
        if reusable.is_none() {
            let _ = revoke(&record);
        }
        return Err(error);
    }
    response.remove("capability");
    Ok(OpenOutcome { record, reused, response })
}

/// Ask authenticated bootstrap state what became of a session.
pub fn reconcile(store: &SessionStore, id: &SessionId) -> Result<Reconciliation, InspectionError> {
    let record = require_record(store, id)?;
    if record.state == SessionState::Closed {
        return Ok(Reconciliation::Revoked);
    }
    reconcile_record(store, &record)
}

/// Revoke a session's access, or record that revocation could not be confirmed.
pub fn close(store: &SessionStore, id: &SessionId) -> Result<CloseOutcome, InspectionError> {
    let mut record = require_record(store, id)?;
    if record.state == SessionState::Closed {
        return Ok(CloseOutcome::AlreadyClosed);
    }

    if record.state == SessionState::CloseUnconfirmed {
        match reconcile_record(store, &record)? {
            Reconciliation::Revoked => {
                return Ok(CloseOutcome::ReconciledClosed { process_exited: false });
            }
            Reconciliation::ProcessExited => {
                return Ok(CloseOutcome::ReconciledClosed { process_exited: true });
            }
            Reconciliation::Unreachable(cause) => return Ok(CloseOutcome::Unconfirmed(cause)),
            // Still active: the earlier close never took effect, so ask again.
            Reconciliation::Active => {}
        }
    }

    match revoke(&record) {
        Ok(response) => {
            let was_active = response.get("wasActive").and_then(Value::as_bool).unwrap_or(true);
            mark(store, &mut record, SessionState::Closed)?;
            Ok(CloseOutcome::Acknowledged { was_active })
        }
        // The helper, or the kernel's view of who holds the endpoint, says this session is gone.
        Err(error) if error.code() == InspectionErrorCode::StaleSession => {
            let process_exited = error.stage() == Stage::Peer;
            if process_exited {
                store.end_execution(record.incarnation)?;
            }
            mark(store, &mut record, SessionState::Closed)?;
            Ok(CloseOutcome::ReconciledClosed { process_exited })
        }
        Err(error) if error.leaves_outcome_unknown() => {
            if process_absent(record.incarnation) {
                store.end_execution(record.incarnation)?;
                mark(store, &mut record, SessionState::Closed)?;
                return Ok(CloseOutcome::ReconciledClosed { process_exited: true });
            }
            mark(store, &mut record, SessionState::CloseUnconfirmed)?;
            Ok(CloseOutcome::Unconfirmed(error))
        }
        Err(error) => Err(error),
    }
}

/// Carry one native read to the helper under the process-wide read guard.
///
/// `params` supplies the requested scope, fields, and bounds; this layer neither interprets nor
/// filters them. The guard is released when the read resolves, and deliberately left in place when
/// a deadline expires, because the target's main thread may still be running that read.
pub fn native_request(
    store: &SessionStore,
    id: &SessionId,
    op: Op,
    params: Value,
) -> Result<Map<String, Value>, InspectionError> {
    if op.is_lifecycle() {
        return Err(InspectionError::new(
            InspectionErrorCode::InvalidArgument,
            Stage::Send,
            format!("`{}` is a lifecycle operation, not a native read", op.as_wire()),
        ));
    }
    let record = require_record(store, id)?;
    match record.state {
        SessionState::Active => {}
        SessionState::CloseUnconfirmed => {
            return Err(InspectionError::new(
                InspectionErrorCode::Busy,
                Stage::Store,
                format!(
                    "whether session {} was revoked is unknown; reconcile or close it before reading",
                    record.session_id
                ),
            ));
        }
        SessionState::Closed => {
            return Err(InspectionError::new(
                InspectionErrorCode::StaleSession,
                Stage::Store,
                format!("session {} is closed", record.session_id),
            ));
        }
    }

    store.begin_execution(record.incarnation, op, &record.session_id)?;
    let result = dispatch_native(&record, op, params);
    // A helper answer — success or typed failure — proves the main thread returned. Only an
    // unresolved deadline keeps the guard.
    if result.as_ref().err().is_none_or(|error| !error.may_continue_in_target()) {
        store.end_execution(record.incarnation)?;
    }
    result
}

fn dispatch_native(
    record: &SessionRecord,
    op: Op,
    params: Value,
) -> Result<Map<String, Value>, InspectionError> {
    let Value::Object(params) = params else {
        return Err(InspectionError::new(
            InspectionErrorCode::InvalidArgument,
            Stage::Send,
            "native request options must be a JSON object",
        ));
    };
    let mut connection = Connection::connect(&record.socket_path)?;
    connection.require_incarnation(record.incarnation)?;

    let mut body = envelope(record.protocol, &record.helper_build)?;
    body.insert("sessionId".to_owned(), Value::from(record.session_id.as_str()));
    body.insert("capability".to_owned(), Value::from(record.capability.expose()));
    body.extend(params);
    connection.request(op, Value::Object(body))
}

fn revoke(record: &SessionRecord) -> Result<Map<String, Value>, InspectionError> {
    let mut connection = Connection::connect(&record.socket_path)?;
    connection.require_incarnation(record.incarnation)?;

    let mut body = envelope(record.protocol, &record.helper_build)?;
    body.insert("sessionId".to_owned(), Value::from(record.session_id.as_str()));
    body.insert("capability".to_owned(), Value::from(record.capability.expose()));
    connection.request(Op::Close, Value::Object(body))
}

/// Resolve a session's fate against bootstrap state, recording a confirmed outcome.
///
/// Reconciliation presents no capability: after an unacknowledged close the host holds one it
/// cannot trust, and bootstrap must answer "is this session still yours" without it.
fn reconcile_record(
    store: &SessionStore,
    record: &SessionRecord,
) -> Result<Reconciliation, InspectionError> {
    let mut working = record.clone();
    if process_absent(record.incarnation) {
        store.end_execution(record.incarnation)?;
        mark(store, &mut working, SessionState::Closed)?;
        return Ok(Reconciliation::ProcessExited);
    }

    let mut connection = match Connection::connect(&record.socket_path) {
        Ok(connection) => connection,
        Err(error) => {
            if process_absent(record.incarnation) {
                store.end_execution(record.incarnation)?;
                mark(store, &mut working, SessionState::Closed)?;
                return Ok(Reconciliation::ProcessExited);
            }
            return Ok(Reconciliation::Unreachable(error));
        }
    };
    // A different incarnation now holds the endpoint, so the bound process is gone whatever the
    // pid currently names.
    if connection.peer().incarnation != record.incarnation {
        store.end_execution(record.incarnation)?;
        mark(store, &mut working, SessionState::Closed)?;
        return Ok(Reconciliation::ProcessExited);
    }

    let mut body = match envelope(record.protocol, &record.helper_build) {
        Ok(body) => body,
        Err(error) => return Ok(Reconciliation::Unreachable(error)),
    };
    body.insert("sessionId".to_owned(), Value::from(record.session_id.as_str()));

    let response = match connection.request(Op::Reconcile, Value::Object(body)) {
        Ok(response) => response,
        Err(error) => return Ok(Reconciliation::Unreachable(error)),
    };

    // Only the helper can retire the read guard: it alone knows whether the dispatched read
    // returned.
    if response.get("outstandingRead").and_then(Value::as_bool) == Some(false) {
        store.end_execution(record.incarnation)?;
    }

    let active = response.get("active").and_then(Value::as_bool).unwrap_or(false);
    let names_this_session = response
        .get("sessionId")
        .and_then(Value::as_str)
        .is_none_or(|reported| reported == record.session_id.as_str());
    if active && names_this_session {
        Ok(Reconciliation::Active)
    } else {
        mark(store, &mut working, SessionState::Closed)?;
        Ok(Reconciliation::Revoked)
    }
}

/// The fields every request carries: the negotiated protocol and build, and this process's own
/// kernel-verified identity for the helper to check against its view of the connection.
fn envelope(protocol: u32, helper_build: &str) -> Result<Map<String, Value>, InspectionError> {
    let client = current_incarnation()?;
    let mut body = Map::new();
    body.insert("protocol".to_owned(), Value::from(protocol));
    body.insert("helperBuild".to_owned(), Value::from(helper_build));
    body.insert("clientPid".to_owned(), Value::from(client.pid));
    body.insert("clientPidversion".to_owned(), Value::from(client.pidversion));
    if let Some(owner) = current_uid() {
        body.insert("clientUid".to_owned(), Value::from(owner));
    }
    Ok(body)
}

fn require_compatible(
    response: &Map<String, Value>,
    protocol: u32,
    helper_build: &str,
) -> Result<(), InspectionError> {
    let reported_protocol = response.get("protocol").and_then(Value::as_u64);
    if reported_protocol != Some(u64::from(protocol)) {
        return Err(InspectionError::new(
            InspectionErrorCode::ProtocolMismatch,
            Stage::Handshake,
            format!(
                "the helper answered on protocol {}, not {protocol}",
                reported_protocol.map_or_else(
                    || "an unreported version".to_owned(),
                    |version| version.to_string()
                )
            ),
        ));
    }
    let reported_build = response.get("helperBuild").and_then(Value::as_str);
    if reported_build != Some(helper_build) {
        return Err(InspectionError::new(
            InspectionErrorCode::ProtocolMismatch,
            Stage::Handshake,
            format!(
                "the helper identifies as build {}, not {helper_build}",
                reported_build.unwrap_or("an unreported build")
            ),
        ));
    }
    Ok(())
}

/// A handshake that answers for another process cannot establish a session, whatever it claims.
fn require_incarnation(
    response: &Map<String, Value>,
    expected: Incarnation,
) -> Result<(), InspectionError> {
    let pid = response.get("pid").and_then(Value::as_i64);
    let pidversion = response.get("pidversion").and_then(Value::as_u64);
    if pid == Some(i64::from(expected.pid)) && pidversion == Some(u64::from(expected.pidversion)) {
        Ok(())
    } else {
        Err(InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Handshake,
            format!(
                "the helper reported pid {} generation {}, not {expected}",
                pid.map_or_else(|| "none".to_owned(), |pid| pid.to_string()),
                pidversion.map_or_else(|| "none".to_owned(), |version| version.to_string())
            ),
        ))
    }
}

fn string_field<'a>(
    response: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, InspectionError> {
    response.get(field).and_then(Value::as_str).ok_or_else(|| {
        InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Handshake,
            format!("the helper response carried no `{field}`"),
        )
    })
}

fn require_record(store: &SessionStore, id: &SessionId) -> Result<SessionRecord, InspectionError> {
    store.record(id)?.ok_or_else(|| {
        InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Store,
            format!("session {id} is not recorded for this user"),
        )
    })
}

fn mark(
    store: &SessionStore,
    record: &mut SessionRecord,
    state: SessionState,
) -> Result<(), InspectionError> {
    record.state = state;
    record.updated_at = now_unix();
    store.save(record)
}

/// Whether the pid names no process at all. A pid that has been reused is present, so this only
/// ever reports a confirmed absence.
#[cfg(target_os = "macos")]
fn process_absent(incarnation: Incarnation) -> bool {
    // SAFETY: signal 0 performs the permission and existence checks without delivering a signal.
    if unsafe { libc::kill(incarnation.pid, 0) } == 0 {
        return false;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
}

#[cfg(not(target_os = "macos"))]
fn process_absent(_incarnation: Incarnation) -> bool {
    false
}
