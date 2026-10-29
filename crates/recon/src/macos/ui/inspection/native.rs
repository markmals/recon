//! Native visual requests and session/target validation shared with external UI commands.

use serde_json::{Map, Value, json};

use super::session::{default_store, same_bundle};
use crate::application::{Application, Pid};
use crate::inspection::{
    InspectionError, InspectionErrorCode, Op, Reconciliation, SessionId, SessionRecord,
    SessionState, Stage, native_request, reconcile,
};
use crate::ui::request::{FieldSet, Filters, NativeScope, TraversalKind, ViewRequest};

// Native reads authenticate under their own two-second deadline; do not spend a
// separate ten-second reconciliation round trip before sending them.
pub(crate) fn session_record(
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

pub(crate) fn validate_session(
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

pub(crate) fn view(
    application: &Application,
    pid: Pid,
    request: &ViewRequest,
) -> Result<Value, InspectionError> {
    let (session, op, params) = match request {
        ViewRequest::Tree { selection: _, session, scope, kind, depth, fields, limit } => {
            let mut params = scope_params(scope);
            params.insert("kind".to_owned(), Value::from(kind.as_str()));
            params.insert("depth".to_owned(), Value::from(*depth));
            params.insert("limit".to_owned(), Value::from(*limit));
            params.insert("fields".to_owned(), fields_value(fields)?);
            (session, Op::Tree, params)
        }
        ViewRequest::Node { selection: _, session, handle, fields } => {
            let mut params = Map::new();
            params.insert("root".to_owned(), Value::from(handle.as_str()));
            params.insert("fields".to_owned(), fields_value(fields)?);
            (session, Op::Node, params)
        }
        ViewRequest::Find { selection: _, session, scope, kind, filters, depth, fields, limit } => {
            let mut params = scope_params(scope);
            params.insert("kind".to_owned(), Value::from(kind.as_str()));
            params.insert("depth".to_owned(), Value::from(*depth));
            params.insert("limit".to_owned(), Value::from(*limit));
            params.insert("fields".to_owned(), fields_value(fields)?);
            filter_params(&mut params, filters);
            (session, Op::Find, params)
        }
        ViewRequest::Fonts { selection: _, session, scope, depth, limit } => {
            let mut params = scope_params(scope);
            params.insert("kind".to_owned(), Value::from(TraversalKind::View.as_str()));
            params.insert("depth".to_owned(), Value::from(*depth));
            params.insert("limit".to_owned(), Value::from(*limit));
            (session, Op::Fonts, params)
        }
    };
    session_record(application, Some(pid), session)?;
    let store = default_store()?;
    native_request(&store, session, op, Value::Object(params)).map(Value::Object)
}

pub(crate) fn native_windows(
    application: &Application,
    pid: Pid,
    session: &SessionId,
) -> Result<Value, InspectionError> {
    session_record(application, Some(pid), session)?;
    let store = default_store()?;
    native_request(&store, session, Op::Windows, json!({})).map(Value::Object)
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
