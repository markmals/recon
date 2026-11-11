mod click;
mod key;

use std::time::Instant;

use recon_accessibility_macos::window_element::focused_window_id;
use recon_applications_macos::application::Application;
use recon_core::error::{
    ActionFailure, ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};
use recon_core::process::Pid;
use recon_input_macos::request::{ClickMethod, Location, MouseButton};
use recon_input_macos::{Target, require_unlocked, scroll};
use recon_native_access_macos::identity::SessionId;
use recon_privacy_macos::permission::Permission;
use recon_ui_guard_macos::Watch;
use recon_window_server_macos::window::WindowId;

use crate::observe::target::{validate_session, with_instance};
use crate::ui::interaction_response::ActionApplication;
use crate::ui::request::UiRequest;

pub(super) fn preflight(
    request: &UiRequest,
    started: Instant,
) -> Result<Option<ClickMethod>, Failure> {
    recon_accessibility_macos::element::bound_messaging();
    match request {
        UiRequest::Click {
            selection,
            session,
            window,
            location,
            method,
            button,
            count,
            modifiers,
        } => with_instance(selection, session.as_ref(), |_, pid| {
            let target = Target::bind(pid, *window, ActionMechanism::None)?;
            recon_privacy_macos::permissions::require(&[Permission::Accessibility])?;
            let mut prepared = None;
            let needs_posting = match (method, location) {
                (ClickMethod::Ax, _) => false,
                (ClickMethod::Mouse, _) | (_, Location::Point(_)) => true,
                (ClickMethod::Auto, Location::Element(selector)) => {
                    let plain = *button == MouseButton::Left && *count == 1 && modifiers.is_empty();
                    let bounds = target.window().clone();
                    let element = recon_accessibility_macos::interaction::resolve(
                        pid,
                        &bounds,
                        selector,
                        recon_accessibility_macos::interaction::ResolvePurpose::Click,
                    )?;
                    let effective = if plain && element.advertises_press() {
                        ClickMethod::Ax
                    } else {
                        ClickMethod::Mouse
                    };
                    prepared = Some(effective);
                    effective == ClickMethod::Mouse
                }
            };
            if needs_posting {
                recon_privacy_macos::permissions::require(&[Permission::EventPosting])?;
            }
            check_started(started, pid).map(|()| prepared)
        }),
        UiRequest::Scroll { .. } | UiRequest::Key { .. } => {
            recon_privacy_macos::permissions::require(&[
                Permission::Accessibility,
                Permission::EventPosting,
            ])?;
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn check_started(started: Instant, pid: Pid) -> Result<(), Failure> {
    if started.elapsed() <= std::time::Duration::from_secs(10) {
        return Ok(());
    }
    Err(Failure::new(
        ErrorCode::Timeout,
        "the interaction exceeded its ten-second deadline before dispatch",
        serde_json::json!({"pid": pid.0, "timeoutSeconds": 10}),
    ))
}

pub(super) fn run(
    request: &UiRequest,
    watch: Watch,
    started: Instant,
    prepared_click: Option<ClickMethod>,
) -> Result<(String, ReleaseState), Failure> {
    match request {
        UiRequest::Click {
            selection,
            session,
            window,
            location,
            method,
            button,
            count,
            modifiers,
        } => with_instance(selection, session.as_ref(), |application, pid| {
            click::run(
                application,
                pid,
                *window,
                location,
                *method,
                *button,
                *count,
                *modifiers,
                session.as_ref(),
                &watch,
                started,
                prepared_click,
            )
        }),
        UiRequest::Scroll { selection, session, window, location, dx, dy, unit } => {
            with_instance(selection, session.as_ref(), |application, pid| {
                let mut context = Context::prepare(
                    application,
                    pid,
                    *window,
                    ActionMechanism::ScrollWheel,
                    session.as_ref(),
                    started,
                )?;
                let point = context.point(location)?;
                context.permissions(&[Permission::EventPosting])?;
                context.check(DispatchState::None, ReleaseState::NotNeeded, false)?;
                if watch.interrupted() {
                    return Err(context.stopped(DispatchState::None, ReleaseState::NotNeeded));
                }
                let release = scroll(&context.target, point, *dx, *dy, *unit)?;
                context.check(DispatchState::Complete, release, true)?;
                let changed = context.changed(DispatchState::Complete, release)?;
                let encoded =
                    crate::output::encode(&crate::ui::interaction_response::ScrollResponse {
                        application: context.application(),
                        window: *window,
                        operation: crate::ui::interaction_response::ActionOperation::Scroll,
                        mechanism: ActionMechanism::ScrollWheel,
                        outcome: crate::ui::interaction_response::Outcome::Unverified,
                        delta: crate::ui::interaction_response::ScrollDelta {
                            x: *dx,
                            y: *dy,
                            unit: *unit,
                        },
                        target_key_window_changed: changed,
                        release,
                    })?;
                Ok((encoded, release))
            })
        }
        UiRequest::Key { selection, session, window, chord } => {
            with_instance(selection, session.as_ref(), |application, pid| {
                key::run(application, pid, *window, chord, session.as_ref(), &watch, started)
            })
        }
        _ => unreachable!("interaction request"),
    }
}

pub(super) struct Context<'a> {
    application: &'a Application,
    pub target: Target,
    mechanism: ActionMechanism,
    before: Option<WindowId>,
    started: Instant,
}

impl<'a> Context<'a> {
    fn prepare(
        application: &'a Application,
        pid: Pid,
        window: WindowId,
        mechanism: ActionMechanism,
        session: Option<&SessionId>,
        started: Instant,
    ) -> Result<Self, Failure> {
        if let Some(session) = session {
            validate_session(application, pid, session)?;
        }
        let target = Target::bind(pid, window, mechanism)?;
        require_unlocked(&target, mechanism)?;
        recon_privacy_macos::permissions::require(&[Permission::Accessibility])?;
        let before = focused_window_id(pid)?;
        Ok(Self { application, target, mechanism, before, started })
    }

    fn permissions(&self, needed: &[Permission]) -> Result<(), Failure> {
        recon_privacy_macos::permissions::require(needed)
    }

    fn point(
        &mut self,
        location: &recon_input_macos::request::Location,
    ) -> Result<recon_input_macos::request::Point, Failure> {
        match location {
            recon_input_macos::request::Location::Point(point) => {
                Target::validate_point(self.target.window(), *point, self.mechanism)?;
                Ok(*point)
            }
            recon_input_macos::request::Location::Element(selector) => {
                self.permissions(&[Permission::Accessibility])?;
                let window = self.target.revalidate(
                    self.mechanism,
                    DispatchState::None,
                    ReleaseState::NotNeeded,
                )?;
                let element = recon_accessibility_macos::interaction::resolve(
                    self.target.pid(),
                    &window,
                    selector,
                    recon_accessibility_macos::interaction::ResolvePurpose::Scroll,
                )?;
                Ok(element_point(&element, &window))
            }
        }
    }

    fn application(&self) -> ActionApplication<'a> {
        ActionApplication {
            path: &self.application.path,
            pid: self.target.pid().0,
            pidversion: self.target.incarnation().pidversion,
        }
    }

    fn changed(&self, dispatch: DispatchState, release: ReleaseState) -> Result<bool, Failure> {
        focused_window_id(self.target.pid()).map(|after| after != self.before).map_err(|error| {
            if error.code == ErrorCode::Timeout {
                self.failure(
                    ErrorCode::ActionTimeout,
                    "the target's key window could not be read after the action; execution is unknown",
                    ActionStage::Dispatch,
                    dispatch,
                    self.mechanism,
                    release,
                )
            } else {
                error
            }
        })
    }

    fn stopped(&self, dispatch: DispatchState, release: ReleaseState) -> Failure {
        self.failure(
            ErrorCode::ActionFailed,
            "the interaction guard stopped the operation before its next mutation",
            ActionStage::Dispatch,
            dispatch,
            self.mechanism,
            release,
        )
    }

    fn check(
        &self,
        dispatch: DispatchState,
        release: ReleaseState,
        changed: bool,
    ) -> Result<(), Failure> {
        if self.started.elapsed() <= std::time::Duration::from_secs(10) {
            return Ok(());
        }
        if !changed {
            return Err(Failure::new(
                ErrorCode::Timeout,
                "the interaction exceeded its ten-second deadline before any change",
                serde_json::json!({"pid": self.target.pid().0, "timeoutSeconds": 10}),
            ));
        }
        Err(self.failure(
            ErrorCode::ActionTimeout,
            "the interaction exceeded its ten-second deadline",
            ActionStage::Dispatch,
            dispatch,
            self.mechanism,
            release,
        ))
    }

    fn failure(
        &self,
        code: ErrorCode,
        message: &str,
        stage: ActionStage,
        dispatch: DispatchState,
        mechanism: ActionMechanism,
        release: ReleaseState,
    ) -> Failure {
        Failure::new(
            code,
            message,
            ActionFailure {
                stage,
                dispatch,
                mechanism,
                pid: self.target.pid().0,
                window_id: self.target.window().id.0,
                release,
                key_window: None,
            }
            .details(),
        )
    }
}

pub(super) fn element_point(
    element: &recon_accessibility_macos::interaction::ResolvedElement,
    window: &recon_window_server_macos::window::Window,
) -> recon_input_macos::request::Point {
    recon_input_macos::request::Point {
        x: element.frame.x + element.frame.width / 2.0 - window.bounds.x,
        y: element.frame.y + element.frame.height / 2.0 - window.bounds.y,
    }
}
