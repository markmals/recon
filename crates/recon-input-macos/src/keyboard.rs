use std::thread::sleep;
use std::time::Duration;

use recon_core::error::{ActionMechanism, DispatchState, ErrorCode, Failure, ReleaseState};

use crate::chord::{Chord, Modifiers};
use crate::event::{Delivery, KeyEvent, Source, event_failure, post_key};
use crate::failure::{details, interrupted, with_release};
use crate::target::Target;

#[derive(Clone, Copy)]
struct Step {
    keycode: u16,
    down: bool,
    flags: u64,
    modifier: Option<Modifiers>,
    base: bool,
}

impl Step {
    const EMPTY: Self = Self { keycode: 0, down: false, flags: 0, modifier: None, base: false };
}

pub fn send(
    target: &Target,
    chord: &Chord,
    base_keycode: u16,
    mut interrupted_now: impl FnMut() -> bool,
) -> Result<ReleaseState, Failure> {
    let (steps, length) = sequence(chord, base_keycode);
    let source = Source::private().ok_or_else(|| {
        event_failure(
            ActionMechanism::KeyEvents,
            target,
            DispatchState::None,
            ReleaseState::NotNeeded,
        )
    })?;
    let mut modifiers_down = [false; 4];
    let mut base_down = false;
    for (index, step) in steps[..length].iter().copied().enumerate() {
        let dispatch = if index == 0 { DispatchState::None } else { DispatchState::Partial };
        if interrupted_now() {
            let failure = interrupted(details(
                target,
                ActionMechanism::KeyEvents,
                recon_core::error::ActionStage::Dispatch,
                dispatch,
                ReleaseState::Failed,
            ));
            return Err(release_chord(
                target,
                &source,
                base_keycode,
                &mut modifiers_down,
                &mut base_down,
                failure,
            ));
        }
        let release = if base_down || modifiers_down.iter().any(|down| *down) {
            ReleaseState::Failed
        } else {
            ReleaseState::NotNeeded
        };
        if let Err(failure) = post_key(
            target,
            &source,
            KeyEvent { keycode: step.keycode, down: step.down, flags: step.flags },
            Delivery { dispatch, release },
        ) {
            return Err(if failure.code == ErrorCode::StaleTarget {
                failure
            } else {
                release_chord(
                    target,
                    &source,
                    base_keycode,
                    &mut modifiers_down,
                    &mut base_down,
                    failure,
                )
            });
        }
        if let Some(modifier) = step.modifier {
            let position = Modifiers::ORDER
                .iter()
                .position(|candidate| *candidate == modifier)
                .expect("modifier order");
            modifiers_down[position] = step.down;
        }
        if step.base {
            base_down = step.down;
        }
        if index + 1 < length {
            sleep(Duration::from_millis(30));
        }
    }
    Ok(ReleaseState::Complete)
}

fn sequence(chord: &Chord, base_keycode: u16) -> ([Step; 10], usize) {
    let mut steps = [Step::EMPTY; 10];
    let mut length = 0;
    let mut flags = 0;
    for modifier in
        Modifiers::ORDER.into_iter().filter(|modifier| chord.modifiers.contains(*modifier))
    {
        flags |= modifier.event_flags();
        steps[length] = Step {
            keycode: modifier.key_code(),
            down: true,
            flags,
            modifier: Some(modifier),
            base: false,
        };
        length += 1;
    }
    steps[length] = Step {
        keycode: base_keycode,
        down: true,
        flags: chord.modifiers.event_flags(),
        modifier: None,
        base: true,
    };
    length += 1;
    for modifier in
        Modifiers::ORDER.into_iter().rev().filter(|modifier| chord.modifiers.contains(*modifier))
    {
        flags &= !modifier.event_flags();
        steps[length] = Step {
            keycode: modifier.key_code(),
            down: false,
            flags,
            modifier: Some(modifier),
            base: false,
        };
        length += 1;
    }
    steps[length] =
        Step { keycode: base_keycode, down: false, flags: 0, modifier: None, base: true };
    (steps, length + 1)
}

fn release_chord(
    target: &Target,
    source: &Source,
    base_keycode: u16,
    modifiers_down: &mut [bool; 4],
    base_down: &mut bool,
    failure: Failure,
) -> Failure {
    if failure.code == ErrorCode::StaleTarget {
        return with_release(failure, ReleaseState::SkippedTargetChanged);
    }
    let (events, length) = release_events(base_keycode, modifiers_down, *base_down);
    let mut release = if length == 0 { ReleaseState::NotNeeded } else { ReleaseState::Complete };
    for event in events[..length].iter().copied() {
        if let Err(error) =
            post_key(target, source, event, Delivery { dispatch: DispatchState::Partial, release })
        {
            if error.code == ErrorCode::StaleTarget {
                return with_release(failure, ReleaseState::SkippedTargetChanged);
            }
            release = ReleaseState::Failed;
        }
    }
    with_release(failure, release)
}

fn release_events(
    base_keycode: u16,
    modifiers_down: &[bool; 4],
    base_down: bool,
) -> ([KeyEvent; 5], usize) {
    let empty = KeyEvent { keycode: 0, down: false, flags: 0 };
    let mut events = [empty; 5];
    let mut length = 0;
    let mut flags = Modifiers::ORDER
        .into_iter()
        .zip(modifiers_down.iter())
        .filter(|(_, down)| **down)
        .fold(0, |flags, (modifier, _)| flags | modifier.event_flags());
    for position in (0..4).rev() {
        if !modifiers_down[position] {
            continue;
        }
        let modifier = Modifiers::ORDER[position];
        flags &= !modifier.event_flags();
        events[length] = KeyEvent { keycode: modifier.key_code(), down: false, flags };
        length += 1;
    }
    if base_down {
        events[length] = KeyEvent { keycode: base_keycode, down: false, flags: 0 };
        length += 1;
    }
    (events, length)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal0007_physical_chord_releases_modifiers_before_unflagged_base_up() {
        let chord = Chord::parse("cmd+ctrl+shift+b").unwrap();
        let (steps, length) = sequence(&chord, 11);
        let expected = [
            (55, true, Modifiers::COMMAND.event_flags()),
            (59, true, Modifiers::COMMAND.event_flags() | Modifiers::CONTROL.event_flags()),
            (56, true, chord.modifiers.event_flags()),
            (11, true, chord.modifiers.event_flags()),
            (56, false, Modifiers::COMMAND.event_flags() | Modifiers::CONTROL.event_flags()),
            (59, false, Modifiers::COMMAND.event_flags()),
            (55, false, 0),
            (11, false, 0),
        ];
        assert_eq!(length, expected.len());
        for (step, (keycode, down, flags)) in steps[..length].iter().zip(expected) {
            assert_eq!((step.keycode, step.down, step.flags), (keycode, down, flags));
        }
    }

    #[test]
    fn proposal0007_failure_releases_held_modifiers_then_the_actual_base_key() {
        let held = [true, false, false, true];
        let (events, length) = release_events(11, &held, true);
        let expected = [(56, Modifiers::COMMAND.event_flags()), (55, 0), (11, 0)];
        assert_eq!(length, expected.len());
        for (event, (keycode, flags)) in events[..length].iter().zip(expected) {
            assert!(!event.down);
            assert_eq!((event.keycode, event.flags), (keycode, flags));
        }
    }
}
