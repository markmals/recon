//! The AppKit extension as one open session reaches it: dispatch, inputs, and session checks.

use recon_applications_macos::application::Application;
use recon_core::action::{ActionId, Extension, Invocation};
use recon_core::error::Failure;
use recon_core::process::Pid;
use serde_json::{Map, Value};

use crate::host::failure;
use crate::identity::SessionId;
use crate::lifecycle::{default_store, same_bundle};
use crate::request::{FieldSet, Filters, NativeScope, NodeHandle, TraversalKind};
use crate::{
    InspectionError, InspectionErrorCode, Op, Reconciliation, SessionRecord, SessionState, Stage,
    native_request, reconcile,
};

use super::{FIND, FONTS, NODE, TREE, WINDOWS};

/// Every action this extension exports, in the order it declares them.
const EXPORTS: [ActionId; 5] = [WINDOWS, TREE, NODE, FIND, FONTS];

/// The AppKit extension as it is reachable in one open session.
///
/// The session is the access relationship the actions run inside, so it belongs to the extension
/// rather than to an invocation: a handle minted in one session means nothing in another.
pub struct AppKit<'a> {
    application: &'a Application,
    pid: Pid,
    session: &'a SessionId,
}

impl<'a> AppKit<'a> {
    #[must_use]
    pub fn new(application: &'a Application, pid: Pid, session: &'a SessionId) -> Self {
        Self { application, pid, session }
    }
}

impl Extension for AppKit<'_> {
    fn exports(&self) -> &'static [ActionId] {
        &EXPORTS
    }

    fn run(&self, invocation: Invocation) -> Result<Map<String, Value>, Failure> {
        let operation = operation_for(invocation.action);
        self.read(operation, invocation.input)
            .map_err(|error| failure(&error, Some(self.session), Some(self.pid)))
    }
}

impl AppKit<'_> {
    fn read(
        &self,
        operation: Op,
        input: Map<String, Value>,
    ) -> Result<Map<String, Value>, InspectionError> {
        session_record(self.application, Some(self.pid), self.session)?;
        let store = default_store()?;
        native_request(&store, self.session, operation, Value::Object(input))
    }
}

/// The wire operation that carries one exported action.
///
/// The helper protocol's spellings are fixed by proposal.0005 and stay byte-compatible; they are
/// a transport detail of this adapter rather than the action model.
fn operation_for(action: ActionId) -> Op {
    match action.name().as_str() {
        "windows" => Op::Windows,
        "tree" => Op::Tree,
        "node" => Op::Node,
        "find" => Op::Find,
        "fonts" => Op::Fonts,
        name => unreachable!("{name} is not an action this extension exports"),
    }
}

/// What one `ui view` command asks the AppKit extension to read.
pub enum ViewQuery<'a> {
    Tree {
        scope: &'a NativeScope,
        kind: TraversalKind,
        depth: usize,
        fields: &'a FieldSet,
        limit: usize,
    },
    Node {
        handle: &'a NodeHandle,
        fields: &'a FieldSet,
    },
    Find {
        scope: &'a NativeScope,
        kind: TraversalKind,
        filters: &'a Filters,
        depth: usize,
        fields: &'a FieldSet,
        limit: usize,
    },
    Fonts {
        scope: &'a NativeScope,
        depth: usize,
        limit: usize,
    },
}

impl ViewQuery<'_> {
    /// The action's serialized input, in the spellings the helper protocol publishes.
    ///
    /// # Errors
    ///
    /// [`recon_core::error::ErrorCode::InvalidArgument`] when the field groups cannot be encoded.
    pub fn input(&self) -> Result<Map<String, Value>, Failure> {
        self.encoded().map_err(|error| failure(&error, None, None))
    }

    fn encoded(&self) -> Result<Map<String, Value>, InspectionError> {
        Ok(match self {
            Self::Tree { scope, kind, depth, fields, limit } => {
                let mut input = scope_params(scope);
                input.insert("kind".to_owned(), Value::from(kind.as_str()));
                input.insert("depth".to_owned(), Value::from(*depth));
                input.insert("limit".to_owned(), Value::from(*limit));
                input.insert("fields".to_owned(), fields_value(fields)?);
                input
            }
            Self::Node { handle, fields } => {
                let mut input = Map::new();
                input.insert("root".to_owned(), Value::from(handle.as_str()));
                input.insert("fields".to_owned(), fields_value(fields)?);
                input
            }
            Self::Find { scope, kind, filters, depth, fields, limit } => {
                let mut input = scope_params(scope);
                input.insert("kind".to_owned(), Value::from(kind.as_str()));
                input.insert("depth".to_owned(), Value::from(*depth));
                input.insert("limit".to_owned(), Value::from(*limit));
                input.insert("fields".to_owned(), fields_value(fields)?);
                filter_params(&mut input, filters);
                input
            }
            Self::Fonts { scope, depth, limit } => {
                let mut input = scope_params(scope);
                input.insert("kind".to_owned(), Value::from(TraversalKind::View.as_str()));
                input.insert("depth".to_owned(), Value::from(*depth));
                input.insert("limit".to_owned(), Value::from(*limit));
                input
            }
        })
    }
}

pub fn session_record(
    application: &Application,
    requested_pid: Option<Pid>,
    session: &SessionId,
) -> Result<SessionRecord, InspectionError> {
    let store = default_store()?;
    let record = store.record(session)?.ok_or_else(|| {
        InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Store,
            format!("session {session} is not recorded"),
        )
    })?;
    if requested_pid.is_some_and(|pid| i64::from(pid.0) != i64::from(record.incarnation.pid))
        || !same_bundle(&record, application)
    {
        return Err(InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Store,
            "the session and selected application or process do not match",
        ));
    }
    let pid = u32::try_from(record.incarnation.pid).ok().map(Pid);
    if !pid.is_some_and(|pid| application.running_pids.contains(&pid)) {
        return Err(InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Store,
            "the session's bound process is not a running instance of this application",
        ));
    }
    match record.state {
        SessionState::Active => {}
        SessionState::CloseUnconfirmed => {
            return Err(InspectionError::new(
                InspectionErrorCode::Busy,
                Stage::Reconcile,
                "the session has an unconfirmed close; reconcile or close it first",
            ));
        }
        SessionState::Closed => {
            return Err(InspectionError::new(
                InspectionErrorCode::StaleSession,
                Stage::Store,
                "the session is closed",
            ));
        }
    }
    Ok(record)
}

pub fn validate_session(
    application: &Application,
    pid: Pid,
    session: &SessionId,
) -> Result<SessionRecord, InspectionError> {
    let record = session_record(application, Some(pid), session)?;
    let store = default_store()?;
    match reconcile(&store, session)? {
        Reconciliation::Active => Ok(record),
        Reconciliation::Revoked | Reconciliation::ProcessExited => Err(InspectionError::new(
            InspectionErrorCode::StaleSession,
            Stage::Reconcile,
            "the helper no longer holds this session",
        )),
        Reconciliation::Unreachable(error) => Err(error),
    }
}

fn scope_params(scope: &NativeScope) -> Map<String, Value> {
    let mut params = Map::new();
    if let Some(window) = scope.window {
        params.insert("windowId".to_owned(), Value::from(window.0));
    }
    if let Some(root) = &scope.root {
        params.insert("root".to_owned(), Value::from(root.as_str()));
    }
    params
}

fn filter_params(params: &mut Map<String, Value>, filters: &Filters) {
    if let Some(class) = &filters.class {
        params.insert("class".to_owned(), Value::from(class.as_str()));
    }
    if let Some(identifier) = &filters.identifier {
        params.insert("identifier".to_owned(), Value::from(identifier.as_str()));
    }
}

fn fields_value(fields: &FieldSet) -> Result<Value, InspectionError> {
    serde_json::to_value(fields).map_err(|error| {
        InspectionError::new(
            InspectionErrorCode::InvalidArgument,
            Stage::Send,
            format!("field groups could not be encoded: {error}"),
        )
    })
}
