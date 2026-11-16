use recon_core::error::{
    ActionFailure, ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};

use crate::target::Target;

pub(crate) fn with_delivery(mut failure: Failure, delivery: crate::event::Delivery) -> Failure {
    if let Some(details) = failure.details.as_mut() {
        details["stage"] = serde_json::json!("dispatch");
        details["dispatch"] =
            serde_json::to_value(delivery.dispatch).expect("dispatch state serializes");
        details["release"] =
            serde_json::to_value(delivery.release).expect("release state serializes");
    }
    failure
}

pub(crate) fn details(
    target: &Target,
    mechanism: ActionMechanism,
    stage: ActionStage,
    dispatch: DispatchState,
    release: ReleaseState,
) -> ActionFailure {
    ActionFailure {
        stage,
        dispatch,
        mechanism,
        pid: target.pid().0,
        window_id: target.window().id.0,
        release,
        key_window: None,
    }
}

pub(crate) fn action(
    code: ErrorCode,
    message: impl Into<String>,
    details: ActionFailure,
) -> Failure {
    Failure::new(code, message, details.details())
}

pub(crate) fn interrupted(details: ActionFailure) -> Failure {
    action(
        ErrorCode::ActionFailed,
        "the interaction guard stopped the operation before its remaining events",
        details,
    )
}

pub(crate) fn with_release(mut failure: Failure, release: ReleaseState) -> Failure {
    if let Some(details) = failure.details.as_mut() {
        details["release"] = serde_json::to_value(release).expect("release state serializes");
    }
    failure
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal0007_down_failure_keeps_actual_delivery_state() {
        let failure = Failure::new(
            ErrorCode::ActionUnavailable,
            "point is outside fresh bounds",
            ActionFailure {
                stage: ActionStage::Resolve,
                dispatch: DispatchState::None,
                mechanism: ActionMechanism::MouseEvents,
                pid: 42,
                window_id: 123,
                release: ReleaseState::NotNeeded,
                key_window: None,
            }
            .details(),
        );
        let failure = with_delivery(
            failure,
            crate::event::Delivery {
                dispatch: DispatchState::Partial,
                release: ReleaseState::Failed,
            },
        );
        let details = failure.details.expect("details");
        assert_eq!(details["stage"], "dispatch");
        assert_eq!(details["dispatch"], "partial");
        assert_eq!(details["release"], "failed");
    }
}
