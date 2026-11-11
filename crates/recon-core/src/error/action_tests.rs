use serde_json::json;

use super::{
    ActionFailure, ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};

#[test]
fn proposal0007_action_errors_are_never_automatically_retryable() {
    for code in [
        ErrorCode::ActionUnavailable,
        ErrorCode::AmbiguousElement,
        ErrorCode::StaleTarget,
        ErrorCode::ActionFailed,
        ErrorCode::ActionTimeout,
    ] {
        assert!(!code.retryable(), "{code:?}");
    }
}

#[test]
fn proposal0007_action_failure_details_include_dispatch_and_release_state() {
    let failure = Failure::new(
        ErrorCode::ActionFailed,
        "mid-chord delivery failed",
        ActionFailure {
            stage: ActionStage::Dispatch,
            dispatch: DispatchState::Partial,
            mechanism: ActionMechanism::KeyEvents,
            pid: 42,
            window_id: 123,
            release: ReleaseState::Complete,
            key_window: None,
        }
        .details(),
    );
    assert_eq!(
        failure.details,
        Some(json!({
            "stage": "dispatch",
            "dispatch": "partial",
            "mechanism": "key-events",
            "pid": 42,
            "windowId": 123,
            "release": "complete"
        }))
    );
}

#[test]
fn proposal0007_key_window_failure_reports_before_after_and_unknown_change() {
    let failure = Failure::new(
        ErrorCode::ActionFailed,
        "the selected window did not become key",
        ActionFailure {
            stage: ActionStage::KeyWindow,
            dispatch: DispatchState::None,
            mechanism: ActionMechanism::KeyEvents,
            pid: 42,
            window_id: 123,
            release: ReleaseState::NotNeeded,
            key_window: None,
        }
        .key_window(Some(122), None, None)
        .details(),
    );
    assert_eq!(
        failure.details.expect("details")["keyWindow"],
        json!({"before": 122, "after": null, "changed": null})
    );
}
