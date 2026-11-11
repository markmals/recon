use std::sync::atomic::{AtomicI64, Ordering};
use std::thread::sleep;
use std::time::Duration;

use recon_core::error::{ActionMechanism, DispatchState, ErrorCode, Failure, ReleaseState};

use crate::chord::Modifiers;
use crate::event::{Delivery, MouseEvent, Source, event_failure, post_mouse};
use crate::failure::{details, interrupted as interrupted_failure, with_release};
use crate::request::{MouseButton, Point};
use crate::target::Target;

static NEXT_CLICK_GROUP: AtomicI64 = AtomicI64::new(1);

pub fn click(
    target: &Target,
    point: Point,
    button: MouseButton,
    count: u8,
    modifiers: Modifiers,
    mut interrupted_now: impl FnMut() -> bool,
) -> Result<ReleaseState, Failure> {
    Target::validate_point(target.window(), point, ActionMechanism::MouseEvents)?;
    let group = NEXT_CLICK_GROUP.fetch_add(1, Ordering::Relaxed);
    let source = Source::private().ok_or_else(|| {
        event_failure(
            ActionMechanism::MouseEvents,
            target,
            DispatchState::None,
            ReleaseState::NotNeeded,
        )
    })?;
    let mut release = ReleaseState::NotNeeded;
    for click_state in 1..=i64::from(count) {
        let down = MouseEvent {
            point,
            button,
            down: true,
            click_state,
            flags: modifiers.event_flags(),
            group,
        };
        let up = MouseEvent { down: false, ..down };
        let dispatch = click_dispatch(click_state);
        if interrupted_now() {
            return Err(interrupted(target, dispatch, release));
        }
        post_mouse(target, &source, down, Delivery { dispatch, release })?;

        sleep(Duration::from_millis(30));
        if interrupted_now() {
            let failure = interrupted(target, DispatchState::Partial, ReleaseState::Failed);
            return Err(release_click(target, &source, up, failure));
        }
        if let Err(failure) = post_mouse(
            target,
            &source,
            up,
            Delivery { dispatch: DispatchState::Partial, release: ReleaseState::Failed },
        ) {
            return Err(if failure.code == ErrorCode::StaleTarget {
                failure
            } else {
                release_click(target, &source, up, failure)
            });
        }
        release = ReleaseState::Complete;
        if click_state < i64::from(count) {
            sleep(Duration::from_millis(60));
            if interrupted_now() {
                return Err(interrupted(target, DispatchState::Partial, release));
            }
        }
    }
    Ok(release)
}

fn release_click(target: &Target, source: &Source, up: MouseEvent, failure: Failure) -> Failure {
    match post_mouse(
        target,
        source,
        up,
        Delivery { dispatch: DispatchState::Partial, release: ReleaseState::Failed },
    ) {
        Ok(()) => with_release(failure, ReleaseState::Complete),
        Err(release_failure) if release_failure.code == ErrorCode::StaleTarget => {
            with_release(failure, ReleaseState::SkippedTargetChanged)
        }
        Err(_) => with_release(failure, ReleaseState::Failed),
    }
}

fn click_dispatch(click_state: i64) -> DispatchState {
    if click_state == 1 { DispatchState::None } else { DispatchState::Partial }
}

fn interrupted(target: &Target, dispatch: DispatchState, release: ReleaseState) -> Failure {
    interrupted_failure(details(
        target,
        ActionMechanism::MouseEvents,
        recon_core::error::ActionStage::Dispatch,
        dispatch,
        release,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal0007_second_click_reports_partial_dispatch() {
        assert_eq!(click_dispatch(1), DispatchState::None);
        assert_eq!(click_dispatch(2), DispatchState::Partial);
    }
}
