use recon_core::error::{ActionMechanism, DispatchState, Failure, ReleaseState};

use crate::event::{Source, event_failure, post_wheel};
use crate::request::{Point, ScrollUnit};
use crate::target::Target;

pub fn scroll(
    target: &Target,
    point: Point,
    dx: i32,
    dy: i32,
    unit: ScrollUnit,
) -> Result<ReleaseState, Failure> {
    Target::validate_point(target.window(), point, ActionMechanism::ScrollWheel)?;
    let source = Source::private().ok_or_else(|| {
        event_failure(
            ActionMechanism::ScrollWheel,
            target,
            DispatchState::None,
            ReleaseState::NotNeeded,
        )
    })?;
    post_wheel(target, &source, point, dx, dy, unit)?;
    Ok(ReleaseState::NotNeeded)
}
