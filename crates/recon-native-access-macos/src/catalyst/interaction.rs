//! The Catalyst extension as one open session reaches it: dispatch, input, and what came back.

use recon_applications_macos::application::Application;
use recon_core::action::{self, ActionId, Extension, Invocation};
use recon_core::error::{
    ActionFailure, ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};
use recon_core::process::Pid;
use recon_window_server_macos::window::WindowId;
use serde_json::{Map, Value};

use crate::appkit::session_record;
use crate::host::failure;
use crate::identity::SessionId;
use crate::lifecycle::default_store;
use crate::{InspectionError, InspectionErrorCode, Op, Stage, native_request};

use super::MAKE_WINDOW_KEY;

/// Every action this extension exports, in the order it declares them.
const EXPORTS: [ActionId; 1] = [MAKE_WINDOW_KEY];

/// UIKit's key window on either side of one make-window-key, as WindowServer numbers.
///
/// A key window UIKit reports as hosted in no WindowServer window, and no key window at all, are
/// both `None`: neither is a window a chord can be addressed to. The caller decides what the
/// pair means — it requires `after` to be the window it selected, and reports a change it made
/// rather than reversing it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalystKeyWindow {
    pub before: Option<WindowId>,
    pub after: Option<WindowId>,
}

/// The Catalyst interaction extension as it is reachable in one open session.
///
/// The session is the access relationship the change is made inside, so it belongs to the
/// extension rather than to an invocation.
pub struct Catalyst<'a> {
    application: &'a Application,
    pid: Pid,
    session: &'a SessionId,
}

impl<'a> Catalyst<'a> {
    #[must_use]
    pub fn new(application: &'a Application, pid: Pid, session: &'a SessionId) -> Self {
        Self { application, pid, session }
    }
}

impl Extension for Catalyst<'_> {
    fn exports(&self) -> &'static [ActionId] {
        &EXPORTS
    }

    fn run(&self, invocation: Invocation) -> Result<Map<String, Value>, Failure> {
        let operation = operation_for(invocation.action);
        self.interact(operation, invocation.input)
            .map_err(|error| failure(&error, Some(self.session), Some(self.pid)))
    }
}

impl Catalyst<'_> {
    fn interact(
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
fn operation_for(action: ActionId) -> Op {
    match action.name().as_str() {
        "makeWindowKey" => Op::MakeWindowKey,
        name => unreachable!("{name} is not an action this extension exports"),
    }
}

/// Make the `UIWindow` hosted in `window` the target's key window, inside the target.
///
/// The change is made by the target's own code on its own main thread, so it is reported rather
/// than assumed: the answer names UIKit's key window before and after, and nothing here decides
/// whether that is the outcome the caller wanted.
///
/// # Errors
///
/// [`recon_core::error::ErrorCode::ActionFailed`] when the helper fails, is unreachable, or
/// answers for a window other than the one selected, and
/// [`recon_core::error::ErrorCode::ActionTimeout`] when the change does not answer within its
/// deadline. Failures of the access relationship itself keep their native-inspection codes.
pub fn make_window_key(
    application: &Application,
    pid: Pid,
    session: &SessionId,
    window: WindowId,
) -> Result<CatalystKeyWindow, Failure> {
    let mut input = Map::new();
    input.insert("windowId".to_owned(), Value::from(window.0));
    action::invoke(
        &Catalyst::new(application, pid, session),
        Invocation { action: MAKE_WINDOW_KEY, input },
    )
    .and_then(|response| {
        reported(&response, window).map_err(|error| failure(&error, Some(session), Some(pid)))
    })
    .map_err(|error| key_window_failure(error, pid, window))
}

/// How one unsuccessful attempt to make the selected window key is published.
///
/// This is the first stage of a mutating action, so what the caller needs is the action
/// vocabulary: which stage failed, that no key event was sent, and that nothing is held down. A
/// helper that failed, that could not be reached, or that answered about another window is
/// therefore `ACTION_FAILED` at stage `key-window`; a change that did not answer within its
/// deadline is `ACTION_TIMEOUT`, because whether the target's key window moved is unknown.
///
/// A failure of the access relationship rather than of the change — a refused argument, a
/// contended or closed session, a resident helper from another build — keeps its own code: what
/// it asks of the operator is about the session, not about the window.
fn key_window_failure(error: Failure, pid: Pid, window: WindowId) -> Failure {
    let native = error.code;
    let code = match native {
        ErrorCode::InspectionFailed | ErrorCode::InspectionUnavailable => ErrorCode::ActionFailed,
        ErrorCode::InspectionTimeout => ErrorCode::ActionTimeout,
        _ => return error,
    };
    Failure::new(
        code,
        error.message,
        ActionFailure {
            stage: ActionStage::KeyWindow,
            dispatch: DispatchState::None,
            mechanism: ActionMechanism::KeyEvents,
            pid: pid.0,
            window_id: window.0,
            release: ReleaseState::NotNeeded,
            key_window: None,
        }
        .details(),
    )
}

fn reported(
    response: &Map<String, Value>,
    requested: WindowId,
) -> Result<CatalystKeyWindow, InspectionError> {
    // The helper resolved exactly one UIWindow for the selected window; an answer about any
    // other window is a protocol violation, not a key-window result.
    if member(response, "window")? != Some(requested) {
        return Err(violation(format!("the helper answered for a window other than {requested}")));
    }
    Ok(CatalystKeyWindow { before: member(response, "before")?, after: member(response, "after")? })
}

/// The WindowServer window one reported UIKit window names, where it names one at all.
fn member(response: &Map<String, Value>, name: &str) -> Result<Option<WindowId>, InspectionError> {
    let reported = response
        .get(name)
        .ok_or_else(|| violation(format!("the make-window-key answer has no `{name}` member")))?;
    let Value::Object(window) = reported else {
        if reported.is_null() {
            return Ok(None);
        }
        return Err(violation(format!("`{name}` is neither a window nor null")));
    };
    match window.get("windowId") {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            let number = value
                .as_u64()
                .and_then(|number| u32::try_from(number).ok())
                .ok_or_else(|| violation(format!("`{name}` names no WindowServer window id")))?;
            Ok(Some(WindowId(number)))
        }
    }
}

fn violation(message: String) -> InspectionError {
    InspectionError::new(InspectionErrorCode::Failed, Stage::Receive, message)
}

#[cfg(test)]
mod interaction_tests {
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;
    use crate::NATIVE_DEADLINE;
    use crate::appkit::WINDOWS;

    const SESSION: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f0";
    /// The target one key chord is addressed to, and the window it selected.
    const PID: Pid = Pid(321);
    const SELECTED: WindowId = WindowId(42);

    fn answer(window: Value, before: Value, after: Value) -> Map<String, Value> {
        let Value::Object(members) = json!({
            "ok": true, "window": window, "before": before, "after": after
        }) else {
            unreachable!("the scripted answer is an object")
        };
        members
    }

    fn window(handle: &str, id: Value) -> Value {
        json!({ "handle": handle, "windowId": id })
    }

    #[test]
    fn the_interaction_runs_in_the_target_under_the_native_deadline() {
        assert_eq!(Op::MakeWindowKey.as_wire(), "makeWindowKey");
        assert!(!Op::MakeWindowKey.is_lifecycle());
        assert_eq!(Op::MakeWindowKey.deadline(), NATIVE_DEADLINE);
        assert_eq!(operation_for(MAKE_WINDOW_KEY), Op::MakeWindowKey);
    }

    #[test]
    fn the_extension_exports_only_make_window_key() {
        assert_eq!(EXPORTS, [MAKE_WINDOW_KEY]);

        let application = Application {
            name: "Fixture".to_owned(),
            bundle_id: Some("com.example.fixture".to_owned()),
            path: PathBuf::from("/Applications/Fixture.app"),
            version: None,
            build: None,
            executable_path: None,
            running_pids: vec![Pid(1)],
        };
        let session = SessionId::parse(SESSION).expect("a well-formed session id");
        let catalyst = Catalyst::new(&application, Pid(1), &session);

        // A read is not reachable through the interaction extension, and refusing it never
        // touches the session store.
        let failure = action::invoke(&catalyst, Invocation { action: WINDOWS, input: Map::new() })
            .expect_err("the interaction extension exports no read");
        assert_eq!(failure.code, ErrorCode::InspectionFailed);
    }

    #[test]
    fn a_well_formed_answer_names_the_key_window_on_both_sides() {
        let reported = reported(
            &answer(window("a", json!(42)), window("b", json!(41)), window("a", json!(42))),
            WindowId(42),
        )
        .expect("a well-formed answer");

        assert_eq!(reported.before, Some(WindowId(41)));
        assert_eq!(reported.after, Some(WindowId(42)));
    }

    #[test]
    fn a_target_with_no_key_window_reports_none() {
        let reported = reported(
            &answer(window("a", json!(7)), Value::Null, window("a", json!(7))),
            WindowId(7),
        )
        .expect("a well-formed answer");

        assert_eq!(reported.before, None);
        assert_eq!(reported.after, Some(WindowId(7)));
    }

    /// A UIKit window hosted in no WindowServer window names no window a chord can reach.
    #[test]
    fn a_key_window_with_no_window_server_window_reports_none() {
        let reported = reported(
            &answer(window("a", json!(7)), window("b", Value::Null), window("a", json!(7))),
            WindowId(7),
        )
        .expect("a well-formed answer");

        assert_eq!(reported.before, None);
    }

    #[test]
    fn an_answer_about_another_window_is_a_protocol_violation() {
        let failure = reported(
            &answer(window("a", json!(43)), Value::Null, window("a", json!(43))),
            WindowId(42),
        )
        .expect_err("the helper answered for another window");

        assert_eq!(failure.code(), InspectionErrorCode::Failed);
        assert_eq!(failure.stage(), Stage::Receive);
        assert!(failure.message().contains("42"), "{failure}");
    }

    #[test]
    fn an_answer_that_names_no_resolved_window_is_a_protocol_violation() {
        let failure =
            reported(&answer(window("a", Value::Null), Value::Null, Value::Null), WindowId(42))
                .expect_err("the resolved window must name the selected one");

        assert_eq!(failure.code(), InspectionErrorCode::Failed);
    }

    #[test]
    fn a_missing_or_malformed_member_is_a_protocol_violation() {
        let mut missing = answer(window("a", json!(5)), Value::Null, window("a", json!(5)));
        missing.remove("after");
        let failure = reported(&missing, WindowId(5)).expect_err("`after` is required");
        assert_eq!(failure.code(), InspectionErrorCode::Failed);
        assert!(failure.message().contains("after"), "{failure}");

        let malformed = answer(window("a", json!(5)), json!("none"), window("a", json!(5)));
        let failure = reported(&malformed, WindowId(5)).expect_err("`before` is not a window");
        assert_eq!(failure.code(), InspectionErrorCode::Failed);

        let negative = answer(window("a", json!(5)), json!({ "windowId": -1 }), Value::Null);
        let failure = reported(&negative, WindowId(5)).expect_err("a window id is unsigned");
        assert_eq!(failure.code(), InspectionErrorCode::Failed);
    }

    /// What `make_window_key` publishes when the session reports `code` at `stage`.
    fn publish(code: InspectionErrorCode, stage: Stage, message: &str) -> Failure {
        let session = SessionId::parse(SESSION).expect("a well-formed session id");
        key_window_failure(
            failure(&InspectionError::new(code, stage, message), Some(&session), Some(PID)),
            PID,
            SELECTED,
        )
    }

    /// The action details every unsuccessful key-window change carries: the stage that failed,
    /// no key event dispatched, nothing held down, and the process and window selected.
    fn key_window_details() -> Value {
        json!({
            "stage": "key-window",
            "dispatch": "none",
            "mechanism": "key-events",
            "pid": 321,
            "windowId": 42,
            "release": "not-needed"
        })
    }

    /// A helper that failed, or that could not be reached, failed to make the window key. The
    /// caller is sending a chord, so it is told the mechanism failed at that stage rather than
    /// being handed the session's own vocabulary.
    #[test]
    fn a_failed_key_window_change_is_an_action_failure_at_that_stage() {
        for code in [InspectionErrorCode::Failed, InspectionErrorCode::Unavailable] {
            let published =
                publish(code, Stage::Read, "the helper resolved no UIWindow for that window");

            assert_eq!(published.code, ErrorCode::ActionFailed, "{code:?}");
            assert!(!published.retryable, "{code:?}");
            assert_eq!(
                published.message, "the helper resolved no UIWindow for that window",
                "{code:?}"
            );
            assert_eq!(published.details, Some(key_window_details()), "{code:?}");
        }
    }

    /// A mutating call that did not answer leaves the target's key window unknown, which is a
    /// different thing to say than a failure.
    #[test]
    fn a_key_window_change_that_did_not_answer_is_an_action_timeout() {
        let published =
            publish(InspectionErrorCode::Timeout, Stage::Read, "the change did not answer");

        assert_eq!(published.code, ErrorCode::ActionTimeout);
        assert_eq!(published.details, Some(key_window_details()));
    }

    /// An answer about another window leaves the change unconfirmed, which is reported the same
    /// way a refused one is: no key event was sent either way.
    #[test]
    fn an_unconfirmed_change_is_an_action_failure_with_no_key_events_sent() {
        let session = SessionId::parse(SESSION).expect("a well-formed session id");
        let violation = reported(
            &answer(window("a", json!(43)), Value::Null, window("a", json!(43))),
            SELECTED,
        )
        .expect_err("the helper answered for another window");
        let published =
            key_window_failure(failure(&violation, Some(&session), Some(PID)), PID, SELECTED);

        assert_eq!(published.code, ErrorCode::ActionFailed);
        assert_eq!(published.details, Some(key_window_details()));
    }

    /// An earlier resident, a contended or closed session, a vanished window, and a refused
    /// argument are about the access relationship rather than the change, so each keeps its own
    /// code and its own details — including the mismatch that tells an operator to open a new
    /// session.
    #[test]
    fn a_session_failure_keeps_its_native_inspection_code() {
        for (code, expected) in [
            (InspectionErrorCode::ProtocolMismatch, ErrorCode::InspectionProtocolMismatch),
            (InspectionErrorCode::Busy, ErrorCode::InspectionBusy),
            (InspectionErrorCode::StaleSession, ErrorCode::StaleSession),
            (InspectionErrorCode::StaleNode, ErrorCode::StaleNode),
            (InspectionErrorCode::InvalidArgument, ErrorCode::InvalidArgument),
        ] {
            let published = publish(code, Stage::Store, "the session cannot serve this request");

            assert_eq!(published.code, expected, "{code:?}");
            let details = published.details.expect("a native-inspection failure has details");
            assert_eq!(details["stage"], json!("store"), "{code:?}");
            assert_eq!(details["session"], json!(SESSION), "{code:?}");
        }
    }
}
