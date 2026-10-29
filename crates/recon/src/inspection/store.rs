//! The owner-private descriptor root: one record per session, plus the per-incarnation endpoint
//! directories and the process-wide native-read guard.
//!
//! Every directory here is 0700 and every file 0600, and both are re-verified on use: a descriptor
//! that someone else can read or write is refused rather than trusted. Records are never silently
//! replaced, because replacing one would discard the uncertainty that an unacknowledged close left
//! behind.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::error::{InspectionError, InspectionErrorCode, Stage};
use super::identity::{AccessProfile, AcquisitionMode, Capability, Incarnation, SessionId};
use super::transport::{Op, current_uid};

/// The on-disk record layout version. A record written by another layout is refused, not guessed at.
const RECORD_VERSION: u32 = 1;

/// How long a confirmed-closed record is kept so a repeated close stays idempotent. After this it
/// is pruned when the next session for any process is recorded.
const CLOSED_RETENTION_SECS: u64 = 7 * 24 * 60 * 60;

/// `sockaddr_un.sun_path` holds 104 bytes including its terminator.
const MAX_SOCKET_PATH_BYTES: usize = 103;

/// What the host knows about a session's access.
///
/// `CloseUnconfirmed` is not a failure state to be cleaned up: it records that revocation was
/// requested and never acknowledged, which must survive until authenticated reconciliation
/// resolves it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    Active,
    CloseUnconfirmed,
    Closed,
}

impl SessionState {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::CloseUnconfirmed => "close-unconfirmed",
            Self::Closed => "closed",
        }
    }

    pub fn from_wire(state: &str) -> Option<Self> {
        match state {
            "active" => Some(Self::Active),
            "close-unconfirmed" => Some(Self::CloseUnconfirmed),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }

    /// Whether a record in this state still describes access that may exist in the target.
    pub fn is_live(self) -> bool {
        matches!(self, Self::Active | Self::CloseUnconfirmed)
    }
}

/// One session's durable identity and state.
///
/// `established_mode` is how access was originally acquired and never changes; `requested_mode` is
/// the mode of the request that last reached the session. Connecting with `existing` therefore
/// cannot rewrite a session that `launch` established.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionRecord {
    pub session_id: SessionId,
    pub capability: Capability,
    pub bundle_path: PathBuf,
    pub bundle_id: String,
    pub incarnation: Incarnation,
    pub established_mode: AcquisitionMode,
    pub requested_mode: AcquisitionMode,
    pub profile: AccessProfile,
    pub helper_build: String,
    pub protocol: u32,
    pub socket_path: PathBuf,
    pub state: SessionState,
    pub opened_at: u64,
    pub updated_at: u64,
}

/// A native read that was dispatched into a target and never resolved.
///
/// It is keyed by process incarnation rather than by session, so closing and reopening a session
/// cannot bypass the guard while the target's main thread may still be running that read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnresolvedExecution {
    pub incarnation: Incarnation,
    pub op: String,
    pub session_id: Option<SessionId>,
    pub since: u64,
}

#[derive(Debug)]
pub struct SessionStore {
    root: PathBuf,
}

impl SessionStore {
    /// Open or create the descriptor root, creating each directory 0700 and refusing one that is
    /// not owner-only.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, InspectionError> {
        let store = Self { root: root.into() };
        if let Some(parent) = store.root.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                InspectionError::from_io(
                    InspectionErrorCode::Unavailable,
                    Stage::Store,
                    &format!("{} could not be created", parent.display()),
                    &error,
                )
            })?;
        }
        for directory in
            [&store.root, &store.sessions_dir(), &store.endpoints_dir(), &store.executions_dir()]
        {
            create_private_dir(directory)?;
        }
        Ok(store)
    }

    /// The default descriptor root for this user.
    pub fn default_root() -> Result<PathBuf, InspectionError> {
        let home = std::env::var_os("HOME").filter(|home| !home.is_empty()).ok_or_else(|| {
            InspectionError::new(
                InspectionErrorCode::Unavailable,
                Stage::Store,
                "HOME is unset, so the owner-private descriptor root has no location",
            )
        })?;
        let relative = if cfg!(target_os = "macos") {
            "Library/Application Support/recon/inspection"
        } else {
            ".local/state/recon/inspection"
        };
        Ok(PathBuf::from(home).join(relative))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Create the 0700 directory the helper's endpoint will live in, and return the endpoint path.
    ///
    /// The host creates the directory so the helper never has to choose a predictable location,
    /// and the result is checked against `sun_path`'s length before anything tries to bind it.
    pub fn endpoint_path(&self, incarnation: Incarnation) -> Result<PathBuf, InspectionError> {
        let directory = self.endpoints_dir().join(incarnation.slug());
        create_private_dir(&directory)?;
        let endpoint = directory.join("sock");
        let length = endpoint.as_os_str().as_encoded_bytes().len();
        if length > MAX_SOCKET_PATH_BYTES {
            return Err(InspectionError::new(
                InspectionErrorCode::Unavailable,
                Stage::Store,
                format!(
                    "the endpoint path is {length} bytes, over the {MAX_SOCKET_PATH_BYTES}-byte limit for a Unix socket"
                ),
            ));
        }
        Ok(endpoint)
    }

    pub fn record(&self, id: &SessionId) -> Result<Option<SessionRecord>, InspectionError> {
        let path = self.record_path(id);
        match read_record(&path) {
            Ok(record) => Ok(Some(record)),
            Err(error) if error.code() == InspectionErrorCode::StaleSession => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Every readable record. A file that does not parse as a record is skipped; a record that is
    /// not owner-only fails the read, because loose permissions are a trust problem rather than a
    /// stray file.
    pub fn records(&self) -> Result<Vec<SessionRecord>, InspectionError> {
        let entries = match fs::read_dir(self.sessions_dir()) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(InspectionError::from_io(
                    InspectionErrorCode::Unavailable,
                    Stage::Store,
                    "the session descriptor directory could not be read",
                    &error,
                ));
            }
        };

        let mut records = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            match read_record(&path) {
                Ok(record) => records.push(record),
                Err(error) if error.code() == InspectionErrorCode::StaleSession => {}
                Err(error) => return Err(error),
            }
        }
        records.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        Ok(records)
    }

    /// The one session for `incarnation` whose access may still exist.
    pub fn live_record_for(
        &self,
        incarnation: Incarnation,
    ) -> Result<Option<SessionRecord>, InspectionError> {
        Ok(self
            .records()?
            .into_iter()
            .find(|record| record.incarnation == incarnation && record.state.is_live()))
    }

    /// Record a newly established session.
    ///
    /// It refuses to displace a live record for the same process, and refuses to reuse an existing
    /// identifier, so neither a helper nor a race can make Recon forget access it still holds.
    pub fn insert(&self, record: &SessionRecord) -> Result<(), InspectionError> {
        if let Some(live) = self.live_record_for(record.incarnation)?
            && live.session_id != record.session_id
        {
            return Err(InspectionError::new(
                InspectionErrorCode::Busy,
                Stage::Store,
                format!(
                    "session {} is still {} for {}; close it before opening another",
                    live.session_id,
                    live.state.as_wire(),
                    live.incarnation
                ),
            ));
        }
        self.prune_closed();

        let path = self.record_path(&record.session_id);
        let mut file =
            OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).map_err(
                |error| {
                    if error.kind() == io::ErrorKind::AlreadyExists {
                        InspectionError::new(
                            InspectionErrorCode::Failed,
                            Stage::Store,
                            format!("session {} is already recorded", record.session_id),
                        )
                    } else {
                        InspectionError::from_io(
                            InspectionErrorCode::Unavailable,
                            Stage::Store,
                            "the session descriptor could not be created",
                            &error,
                        )
                    }
                },
            )?;
        file.write_all(&encode_record(record)?).map_err(|error| {
            InspectionError::from_io(
                InspectionErrorCode::Unavailable,
                Stage::Store,
                "the session descriptor could not be written",
                &error,
            )
        })
    }

    /// Persist a state change to a session that is already recorded.
    pub fn save(&self, record: &SessionRecord) -> Result<(), InspectionError> {
        let path = self.record_path(&record.session_id);
        if !path.exists() {
            return Err(InspectionError::new(
                InspectionErrorCode::StaleSession,
                Stage::Store,
                format!("session {} is not recorded", record.session_id),
            ));
        }
        let temporary = path.with_extension("json.tmp");
        let _ = fs::remove_file(&temporary);
        let mut file =
            OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temporary).map_err(
                |error| {
                    InspectionError::from_io(
                        InspectionErrorCode::Unavailable,
                        Stage::Store,
                        "the session descriptor could not be rewritten",
                        &error,
                    )
                },
            )?;
        file.write_all(&encode_record(record)?).map_err(|error| {
            InspectionError::from_io(
                InspectionErrorCode::Unavailable,
                Stage::Store,
                "the session descriptor could not be written",
                &error,
            )
        })?;
        drop(file);
        fs::rename(&temporary, &path).map_err(|error| {
            InspectionError::from_io(
                InspectionErrorCode::Unavailable,
                Stage::Store,
                "the session descriptor could not be replaced",
                &error,
            )
        })
    }

    /// Remove a record outright. Only a confirmed-closed or confirmed-dead session may be
    /// forgotten; an uncertain one is kept by [`SessionStore::save`] instead.
    pub fn forget(&self, id: &SessionId) -> Result<(), InspectionError> {
        match fs::remove_file(self.record_path(id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(InspectionError::from_io(
                InspectionErrorCode::Unavailable,
                Stage::Store,
                "the session descriptor could not be removed",
                &error,
            )),
        }
    }

    /// Claim the one outstanding native read allowed for `incarnation`.
    ///
    /// The claim is a file rather than process memory because each Recon command is its own
    /// process: an unresolved read from an earlier command must still block this one.
    pub fn begin_execution(
        &self,
        incarnation: Incarnation,
        op: Op,
        session_id: &SessionId,
    ) -> Result<(), InspectionError> {
        let path = self.execution_path(incarnation);
        let outcome = OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path);
        match outcome {
            Ok(mut file) => {
                let claim = StoredExecution {
                    version: RECORD_VERSION,
                    op: op.as_wire().to_owned(),
                    session_id: Some(session_id.as_str().to_owned()),
                    since: now_unix(),
                };
                let encoded = serde_json::to_vec(&claim).map_err(|error| {
                    InspectionError::new(
                        InspectionErrorCode::Failed,
                        Stage::Store,
                        format!("the read guard could not be encoded: {error}"),
                    )
                })?;
                file.write_all(&encoded).map_err(|error| {
                    InspectionError::from_io(
                        InspectionErrorCode::Unavailable,
                        Stage::Store,
                        "the read guard could not be written",
                        &error,
                    )
                })
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                let outstanding = self.unresolved_execution(incarnation)?;
                let detail = outstanding.map_or_else(
                    || "an earlier read".to_owned(),
                    |execution| {
                        format!("a `{}` read dispatched at {}", execution.op, execution.since)
                    },
                );
                Err(InspectionError::new(
                    InspectionErrorCode::Busy,
                    Stage::Store,
                    format!("{detail} for {incarnation} has not resolved; only one helper read runs at a time"),
                )
                .target_execution_may_continue())
            }
            Err(error) => Err(InspectionError::from_io(
                InspectionErrorCode::Unavailable,
                Stage::Store,
                "the read guard could not be created",
                &error,
            )),
        }
    }

    /// Release the read guard. Called when a read resolved — including a helper-reported failure,
    /// which proves the main thread returned.
    pub fn end_execution(&self, incarnation: Incarnation) -> Result<(), InspectionError> {
        match fs::remove_file(self.execution_path(incarnation)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(InspectionError::from_io(
                InspectionErrorCode::Unavailable,
                Stage::Store,
                "the read guard could not be released",
                &error,
            )),
        }
    }

    pub fn unresolved_execution(
        &self,
        incarnation: Incarnation,
    ) -> Result<Option<UnresolvedExecution>, InspectionError> {
        let path = self.execution_path(incarnation);
        let raw = match read_private_file(&path, "read guard") {
            Ok(raw) => raw,
            Err(error) if error.code() == InspectionErrorCode::StaleSession => return Ok(None),
            Err(error) => return Err(error),
        };
        let stored: StoredExecution = match serde_json::from_slice(&raw) {
            Ok(stored) => stored,
            Err(_) => return Ok(None),
        };
        let session_id = stored.session_id.as_deref().and_then(|id| SessionId::parse(id).ok());
        Ok(Some(UnresolvedExecution {
            incarnation,
            op: stored.op,
            session_id,
            since: stored.since,
        }))
    }

    fn sessions_dir(&self) -> PathBuf {
        self.root.join("sessions")
    }

    fn endpoints_dir(&self) -> PathBuf {
        self.root.join("endpoints")
    }

    fn executions_dir(&self) -> PathBuf {
        self.root.join("executions")
    }

    fn record_path(&self, id: &SessionId) -> PathBuf {
        self.sessions_dir().join(format!("{id}.json"))
    }

    fn execution_path(&self, incarnation: Incarnation) -> PathBuf {
        self.executions_dir().join(format!("{}.json", incarnation.slug()))
    }

    /// Drop confirmed-closed records once they are old enough that no caller can still be asking
    /// whether that exact session closed. Best effort: a record that cannot be read is left alone.
    fn prune_closed(&self) {
        let Ok(records) = self.records() else { return };
        let now = now_unix();
        for record in records {
            if record.state == SessionState::Closed
                && now.saturating_sub(record.updated_at) > CLOSED_RETENTION_SECS
            {
                let _ = fs::remove_file(self.record_path(&record.session_id));
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredRecord {
    version: u32,
    session_id: String,
    capability: String,
    bundle_path: String,
    bundle_id: String,
    pid: i32,
    pidversion: u32,
    established_mode: String,
    requested_mode: String,
    profile: String,
    helper_build: String,
    protocol: u32,
    socket_path: String,
    state: String,
    opened_at: u64,
    updated_at: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredExecution {
    version: u32,
    op: String,
    session_id: Option<String>,
    since: u64,
}

fn encode_record(record: &SessionRecord) -> Result<Vec<u8>, InspectionError> {
    let stored = StoredRecord {
        version: RECORD_VERSION,
        session_id: record.session_id.as_str().to_owned(),
        capability: record.capability.expose().to_owned(),
        bundle_path: path_text(&record.bundle_path)?,
        bundle_id: record.bundle_id.clone(),
        pid: record.incarnation.pid,
        pidversion: record.incarnation.pidversion,
        established_mode: record.established_mode.as_wire().to_owned(),
        requested_mode: record.requested_mode.as_wire().to_owned(),
        profile: record.profile.as_wire().to_owned(),
        helper_build: record.helper_build.clone(),
        protocol: record.protocol,
        socket_path: path_text(&record.socket_path)?,
        state: record.state.as_wire().to_owned(),
        opened_at: record.opened_at,
        updated_at: record.updated_at,
    };
    serde_json::to_vec(&stored).map_err(|error| {
        InspectionError::new(
            InspectionErrorCode::Failed,
            Stage::Store,
            format!("the session descriptor could not be encoded: {error}"),
        )
    })
}

/// Read one record. A missing file is reported as `STALE_SESSION` so callers can treat "no such
/// session" separately from "this descriptor cannot be trusted".
fn read_record(path: &Path) -> Result<SessionRecord, InspectionError> {
    let raw = read_private_file(path, "session descriptor")?;
    let stored: StoredRecord = serde_json::from_slice(&raw).map_err(|error| {
        InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Store,
            format!("{} is not a session descriptor: {error}", path.display()),
        )
    })?;
    if stored.version != RECORD_VERSION {
        return Err(InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Store,
            format!("{} was written by descriptor layout {}", path.display(), stored.version),
        ));
    }

    let malformed = |field: &str| {
        InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Store,
            format!("{} has an unreadable {field}", path.display()),
        )
    };
    Ok(SessionRecord {
        session_id: SessionId::parse(&stored.session_id).map_err(|_| malformed("session id"))?,
        capability: Capability::parse(&stored.capability).map_err(|_| malformed("capability"))?,
        bundle_path: PathBuf::from(stored.bundle_path),
        bundle_id: stored.bundle_id,
        incarnation: Incarnation::new(stored.pid, stored.pidversion),
        established_mode: AcquisitionMode::from_wire(&stored.established_mode)
            .ok_or_else(|| malformed("established mode"))?,
        requested_mode: AcquisitionMode::from_wire(&stored.requested_mode)
            .ok_or_else(|| malformed("requested mode"))?,
        profile: AccessProfile::from_wire(&stored.profile).ok_or_else(|| malformed("profile"))?,
        helper_build: stored.helper_build,
        protocol: stored.protocol,
        socket_path: PathBuf::from(stored.socket_path),
        state: SessionState::from_wire(&stored.state).ok_or_else(|| malformed("state"))?,
        opened_at: stored.opened_at,
        updated_at: stored.updated_at,
    })
}

/// Read a file that must be a 0600 regular file owned by this user.
fn read_private_file(path: &Path, kind: &str) -> Result<Vec<u8>, InspectionError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            InspectionError::new(
                InspectionErrorCode::StaleSession,
                Stage::Store,
                format!("{} holds no {kind}", path.display()),
            )
        } else {
            InspectionError::from_io(
                InspectionErrorCode::Unavailable,
                Stage::Store,
                &format!("the {kind} at {} is unreadable", path.display()),
                &error,
            )
        }
    })?;
    verify_private(path, &metadata, 0o600, kind)?;

    let mut raw = Vec::with_capacity(metadata.len() as usize);
    File::open(path).and_then(|mut file| file.read_to_end(&mut raw)).map_err(|error| {
        InspectionError::from_io(
            InspectionErrorCode::Unavailable,
            Stage::Store,
            &format!("the {kind} at {} could not be read", path.display()),
            &error,
        )
    })?;
    Ok(raw)
}

fn create_private_dir(path: &Path) -> Result<(), InspectionError> {
    if let Err(error) = DirBuilder::new().mode(0o700).create(path)
        && error.kind() != io::ErrorKind::AlreadyExists
    {
        return Err(InspectionError::from_io(
            InspectionErrorCode::Unavailable,
            Stage::Store,
            &format!("{} could not be created", path.display()),
            &error,
        ));
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        InspectionError::from_io(
            InspectionErrorCode::Unavailable,
            Stage::Store,
            &format!("{} could not be inspected", path.display()),
            &error,
        )
    })?;
    if !metadata.is_dir() {
        return Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Store,
            format!("{} is not a directory", path.display()),
        ));
    }
    verify_private(path, &metadata, 0o700, "descriptor directory")
}

/// Refuse anything another user can read or write, and anything this user does not own.
fn verify_private(
    path: &Path,
    metadata: &fs::Metadata,
    expected_mode: u32,
    kind: &str,
) -> Result<(), InspectionError> {
    let mode = metadata.permissions().mode() & 0o777;
    if mode != expected_mode {
        return Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Store,
            format!("the {kind} at {} is mode {mode:04o}, not {expected_mode:04o}", path.display()),
        ));
    }
    if let Some(owner) = current_uid()
        && metadata.uid() != owner
    {
        return Err(InspectionError::new(
            InspectionErrorCode::Unavailable,
            Stage::Store,
            format!(
                "the {kind} at {} belongs to uid {}, not to uid {owner}",
                path.display(),
                metadata.uid()
            ),
        ));
    }
    Ok(())
}

fn path_text(path: &Path) -> Result<String, InspectionError> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        InspectionError::new(
            InspectionErrorCode::InvalidArgument,
            Stage::Store,
            format!("{} is not valid UTF-8", path.display()),
        )
    })
}

pub(super) fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs())
}
