use std::thread::sleep;
use std::time::{Duration, Instant};

use recon_applications_macos::application::Application;
use recon_core::error::{
    ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};
use recon_core::process::Pid;
use recon_input_macos::chord::Chord;
use recon_input_macos::{
    Toolkit, frontmost_pid, keycode, make_appkit_key, read_toolkit, require_secure_input_disabled,
    send_chord,
};
use recon_native_access_macos::identity::SessionId;
use recon_privacy_macos::permission::Permission;
use recon_ui_guard_macos::Watch;
use recon_window_server_macos::window::WindowId;

use super::Context;
use crate::ui::interaction_response::{
    ActionOperation, ChordResult, KeyResponse, KeyWindowMechanism, KeyWindowResult, Outcome,
};

pub(super) fn run(
    application: &Application,
    pid: Pid,
    window: WindowId,
    chord: &Chord,
    session: Option<&SessionId>,
    watch: &Watch,
    started: Instant,
) -> Result<(String, ReleaseState), Failure> {
    let context =
        Context::prepare(application, pid, window, ActionMechanism::KeyEvents, session, started)?;
    context.permissions(&[Permission::EventPosting])?;
    require_secure_input_disabled(&context.target)?;
    let executable =
        recon_native_access_macos::running_executable(application, pid).map_err(|error| {
            context.failure(
                ErrorCode::ActionUnavailable,
                &format!("the running target's executable is unavailable: {error}"),
                ActionStage::Resolve,
                DispatchState::None,
                ActionMechanism::KeyEvents,
                ReleaseState::NotNeeded,
            )
        })?;
    let toolkit = read_toolkit(&context.target, &executable)?;
    let base = keycode(&context.target, chord)?;
    let frontmost = frontmost_pid()?;
    if frontmost.is_none() {
        return Err(context.failure(
            ErrorCode::ActionUnavailable,
            "the frontmost application could not be read before a key chord",
            ActionStage::KeyWindow,
            DispatchState::None,
            ActionMechanism::KeyEvents,
            ReleaseState::NotNeeded,
        ));
    }
    if frontmost == Some(pid) && context.before != Some(window) {
        return Err(context.failure(
            ErrorCode::ActionUnavailable,
            "changing a frontmost application's key window would move the user's input",
            ActionStage::KeyWindow,
            DispatchState::None,
            ActionMechanism::KeyEvents,
            ReleaseState::NotNeeded,
        ));
    }

    context.check(DispatchState::None, ReleaseState::NotNeeded, false)?;
    refuse_frontmost_key(&context, window)?;
    let (mechanism, reported_before, after) = match toolkit {
        Toolkit::AppKit => {
            // AXFocusedWindow still names the window that was key after the target is deactivated,
            // when AppKit has no key window at all. An inactive application's focused window is
            // never already key, so the records must run; the result reports no prior key window.
            // The defocus record still targets the focused window, because AppKit restores it as
            // key on reactivation unless it is defocused first.
            let active = recon_accessibility_macos::window_element::application_active(pid)
                .map_err(|error| with_key_window(error, None, None))?;
            let reported_before = if active { context.before } else { None };
            if active && context.before == Some(window) {
                (KeyWindowMechanism::AlreadyKey, reported_before, reported_before)
            } else {
                if let Err(error) =
                    make_appkit_key(&context.target, context.before, || watch.interrupted())
                {
                    let after =
                        recon_accessibility_macos::window_element::focused_window_id(pid).ok();
                    return Err(with_key_window(error, reported_before, after));
                }
                (
                    KeyWindowMechanism::WindowServerRecords,
                    reported_before,
                    confirm_appkit(&context, window, reported_before, watch)?,
                )
            }
        }
        Toolkit::Catalyst => {
            let session = session.ok_or_else(|| {
                context.failure(
                    ErrorCode::ActionUnavailable,
                    "a Mac Catalyst key chord requires an open --session for the same incarnation",
                    ActionStage::KeyWindow,
                    DispatchState::None,
                    ActionMechanism::KeyEvents,
                    ReleaseState::NotNeeded,
                )
            })?;
            if watch.interrupted() {
                return Err(context.failure(
                    ErrorCode::ActionFailed,
                    "the interaction guard stopped Catalyst make-key before it ran",
                    ActionStage::KeyWindow,
                    DispatchState::None,
                    ActionMechanism::KeyEvents,
                    ReleaseState::NotNeeded,
                ));
            }
            let change = recon_native_access_macos::catalyst::make_window_key(
                application,
                pid,
                session,
                window,
            )
            .map_err(|error| with_key_window(error, None, None))?;
            if change.after != Some(window) {
                let failure = context.failure(
                    ErrorCode::ActionFailed,
                    "the helper did not confirm the selected UIKit key window",
                    ActionStage::KeyWindow,
                    DispatchState::None,
                    ActionMechanism::KeyEvents,
                    ReleaseState::NotNeeded,
                );
                return Err(with_key_window(failure, change.before, Some(change.after)));
            }
            let mechanism = if change.before == Some(window) {
                KeyWindowMechanism::AlreadyKey
            } else {
                KeyWindowMechanism::UikitHelper
            };
            (mechanism, change.before, change.after)
        }
    };
    context.check(
        DispatchState::None,
        ReleaseState::NotNeeded,
        mechanism != KeyWindowMechanism::AlreadyKey,
    )?;
    if watch.interrupted() {
        return Err(context.failure(
            ErrorCode::ActionFailed,
            "the interaction guard stopped the chord before its first key event",
            ActionStage::Dispatch,
            DispatchState::None,
            ActionMechanism::KeyEvents,
            ReleaseState::NotNeeded,
        ));
    }
    let release = send_chord(&context.target, chord, base, || watch.interrupted())?;
    context.check(DispatchState::Complete, release, true)?;
    // The key window the operation reported moving from and to are both known: the chord is key
    // input, not a key-window change, so there is no need to read the focused window again after
    // dispatch, and reading it could misreport a delivered chord as a timeout.
    let changed = reported_before != after;
    let encoded = crate::output::encode(&KeyResponse {
        application: context.application(),
        window,
        operation: ActionOperation::Key,
        mechanism: ActionMechanism::KeyEvents,
        outcome: Outcome::Unverified,
        chord: ChordResult { modifiers: chord.modifiers.names(), key: chord.key.name() },
        key_window: KeyWindowResult { mechanism, before: reported_before, after },
        target_key_window_changed: changed,
        release,
    })?;
    Ok((encoded, release))
}

fn confirm_appkit(
    context: &Context<'_>,
    selected: WindowId,
    reported_before: Option<WindowId>,
    watch: &Watch,
) -> Result<Option<WindowId>, Failure> {
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let current = recon_accessibility_macos::window_element::focused_window_id(context.target.pid())
            .map_err(|error| {
                let failure = if error.code == ErrorCode::Timeout {
                    context.failure(
                        ErrorCode::ActionTimeout,
                        "the key-window confirmation timed out after WindowServer records; execution is unknown",
                        ActionStage::KeyWindow,
                        DispatchState::None,
                        ActionMechanism::KeyEvents,
                        ReleaseState::NotNeeded,
                    )
                } else {
                    error
                };
                with_key_window(failure, reported_before, None)
            })?;
        if current == Some(selected) {
            return Ok(current);
        }
        if Instant::now() >= deadline || watch.interrupted() {
            let failure = context.failure(
                ErrorCode::ActionFailed,
                "the selected window did not become the target's key window",
                ActionStage::KeyWindow,
                DispatchState::None,
                ActionMechanism::KeyEvents,
                ReleaseState::NotNeeded,
            );
            return Err(with_key_window(failure, reported_before, Some(current)));
        }
        sleep(Duration::from_millis(40));
    }
}

fn refuse_frontmost_key(context: &Context<'_>, window: WindowId) -> Result<(), Failure> {
    let pid = context.target.pid();
    if frontmost_pid()? == Some(pid)
        && recon_accessibility_macos::window_element::focused_window_id(pid)? != Some(window)
    {
        return Err(context.failure(
            ErrorCode::ActionUnavailable,
            "changing a frontmost application's key window would move the user's input",
            ActionStage::KeyWindow,
            DispatchState::None,
            ActionMechanism::KeyEvents,
            ReleaseState::NotNeeded,
        ));
    }
    Ok(())
}

fn with_key_window(
    mut failure: Failure,
    before: Option<WindowId>,
    after: Option<Option<WindowId>>,
) -> Failure {
    if !matches!(failure.code, ErrorCode::ActionFailed | ErrorCode::ActionTimeout) {
        return failure;
    }
    let changed = after.map(|after| before != after);
    if let Some(details) = failure.details.as_mut() {
        details["keyWindow"] = serde_json::json!({
            "before": before.map(|window| window.0),
            "after": after.map(|window| window.map(|window| window.0)),
            "changed": changed,
        });
    }
    failure
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal0007_action_error_reports_actual_key_window_transition() {
        let failure = Failure::new(
            ErrorCode::ActionFailed,
            "key-window change failed",
            recon_core::error::ActionFailure {
                stage: ActionStage::KeyWindow,
                dispatch: DispatchState::None,
                mechanism: ActionMechanism::KeyEvents,
                pid: 42,
                window_id: 123,
                release: ReleaseState::NotNeeded,
                key_window: None,
            }
            .details(),
        );
        let failure = with_key_window(failure, Some(WindowId(122)), Some(Some(WindowId(123))));
        assert_eq!(
            failure.details.expect("details")["keyWindow"],
            serde_json::json!({"before": 122, "after": 123, "changed": true})
        );
    }
}
