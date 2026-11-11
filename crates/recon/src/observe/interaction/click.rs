use recon_applications_macos::application::Application;
use recon_core::error::{
    ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};
use recon_core::process::Pid;
use recon_input_macos::chord::Modifiers;
use recon_input_macos::frontmost_pid;
use recon_input_macos::request::{ClickMethod, Location, MouseButton};
use recon_native_access_macos::identity::SessionId;
use recon_privacy_macos::permission::Permission;
use recon_ui_guard_macos::Watch;
use recon_window_server_macos::window::WindowId;

use super::{Context, element_point};
use crate::ui::interaction_response::{
    ActionElement, ActionOperation, ClickMethodResult, ClickResponse, LocalPoint, Outcome,
};

#[expect(clippy::too_many_arguments, reason = "one argument per published ui click option")]
pub(super) fn run(
    application: &Application,
    pid: Pid,
    window: WindowId,
    location: &Location,
    requested: ClickMethod,
    button: MouseButton,
    count: u8,
    modifiers: Modifiers,
    session: Option<&SessionId>,
    watch: &Watch,
    started: std::time::Instant,
    prepared: Option<ClickMethod>,
) -> Result<(String, ReleaseState), Failure> {
    let mut context =
        Context::prepare(application, pid, window, ActionMechanism::None, session, started)?;
    let initial = if prepared.is_some() {
        None
    } else {
        match location {
            Location::Element(selector) => {
                context.permissions(&[Permission::Accessibility])?;
                let bounds = context.target.window().clone();
                Some(recon_accessibility_macos::interaction::resolve(
                    pid,
                    &bounds,
                    selector,
                    recon_accessibility_macos::interaction::ResolvePurpose::Click,
                )?)
            }
            Location::Point(_) => None,
        }
    };
    context.check(DispatchState::None, ReleaseState::NotNeeded, false)?;
    let eligible = initial.as_ref().is_some_and(|element| element.advertises_press());
    let plain = button == MouseButton::Left && count == 1 && modifiers.is_empty();
    let effective = match (requested, prepared) {
        (ClickMethod::Auto, Some(method)) => method,
        (ClickMethod::Ax, _) if !eligible => {
            return Err(context.failure(
                ErrorCode::ActionUnavailable,
                "the matched element does not advertise AXPress",
                ActionStage::Resolve,
                DispatchState::None,
                ActionMechanism::AxPress,
                ReleaseState::NotNeeded,
            ));
        }
        (ClickMethod::Ax, _) => ClickMethod::Ax,
        (ClickMethod::Mouse, _) => ClickMethod::Mouse,
        (ClickMethod::Auto, None) if plain && eligible => ClickMethod::Ax,
        (ClickMethod::Auto, _) => ClickMethod::Mouse,
    };
    let reason = match (requested, effective, eligible, plain) {
        (ClickMethod::Ax, _, _, _) | (ClickMethod::Mouse, _, _, _) => "forced",
        (ClickMethod::Auto, ClickMethod::Ax, _, _) => "ax-press-advertised",
        (ClickMethod::Auto, ClickMethod::Mouse, _, _) if matches!(location, Location::Point(_)) => {
            "mouse-required"
        }
        (ClickMethod::Auto, ClickMethod::Mouse, _, false) => "mouse-required",
        (ClickMethod::Auto, ClickMethod::Mouse, _, true) => "ax-press-not-advertised",
        _ => unreachable!("click method selection"),
    };
    let mechanism = if effective == ClickMethod::Ax {
        ActionMechanism::AxPress
    } else {
        ActionMechanism::MouseEvents
    };
    context.mechanism = mechanism;
    if effective == ClickMethod::Mouse {
        context.permissions(&[Permission::EventPosting])?;
        refuse_frontmost_mouse(&context, window)?;
    }

    let dispatch_window =
        context.target.revalidate(mechanism, DispatchState::None, ReleaseState::NotNeeded)?;
    let fresh = match location {
        Location::Element(selector) => {
            let bounds = dispatch_window.clone();
            Some(recon_accessibility_macos::interaction::resolve(
                pid,
                &bounds,
                selector,
                recon_accessibility_macos::interaction::ResolvePurpose::Click,
            )?)
        }
        Location::Point(_) => None,
    };
    context.check(DispatchState::None, ReleaseState::NotNeeded, false)?;
    let point = match (&location, &fresh) {
        (Location::Point(point), _) => *point,
        (Location::Element(_), Some(element)) => element_point(element, &dispatch_window),
        _ => unreachable!("fresh element location"),
    };
    if effective == ClickMethod::Mouse {
        refuse_frontmost_mouse(&context, window)?;
    }
    let release = if effective == ClickMethod::Ax {
        let element = fresh.as_ref().expect("AX click has an element");
        if !element.advertises_press() {
            return Err(context.failure(
                ErrorCode::ActionUnavailable,
                "the live element no longer advertises AXPress",
                ActionStage::Resolve,
                DispatchState::None,
                mechanism,
                ReleaseState::NotNeeded,
            ));
        }
        context.target.revalidate(mechanism, DispatchState::None, ReleaseState::NotNeeded)?;
        if watch.interrupted() {
            return Err(context.stopped(DispatchState::None, ReleaseState::NotNeeded));
        }
        recon_accessibility_macos::interaction::press(element, pid, window)?;
        ReleaseState::NotNeeded
    } else {
        recon_input_macos::click(&context.target, point, button, count, modifiers, || {
            watch.interrupted()
        })?
    };
    context.check(DispatchState::Complete, release, true)?;
    let changed = context.changed(DispatchState::Complete, release)?;
    let encoded = crate::output::encode(&ClickResponse {
        application: context.application(),
        window,
        operation: ActionOperation::Click,
        mechanism,
        outcome: Outcome::Unverified,
        method: ClickMethodResult { requested, effective, reason },
        button,
        count,
        modifiers: modifiers.names(),
        location: LocalPoint { x: point.x, y: point.y },
        element: fresh.map(|element| ActionElement {
            role: element.role,
            title: element.title,
            identifier: element.identifier,
        }),
        target_key_window_changed: changed,
        release,
    })?;
    Ok((encoded, release))
}

fn refuse_frontmost_mouse(context: &Context<'_>, window: WindowId) -> Result<(), Failure> {
    let pid = context.target.pid();
    let frontmost = frontmost_pid()?;
    let key = recon_accessibility_macos::window_element::focused_window_id(pid)?;
    if frontmost == Some(pid) && key != Some(window) {
        return Err(context.failure(
            ErrorCode::ActionUnavailable,
            "a mouse click in a non-key window of the frontmost application would move the user's keyboard focus",
            ActionStage::Resolve,
            DispatchState::None,
            ActionMechanism::MouseEvents,
            ReleaseState::NotNeeded,
        ));
    }
    if frontmost.is_none() {
        return Err(context.failure(
            ErrorCode::ActionUnavailable,
            "the frontmost application could not be read before a mouse click",
            ActionStage::Resolve,
            DispatchState::None,
            ActionMechanism::MouseEvents,
            ReleaseState::NotNeeded,
        ));
    }
    Ok(())
}
