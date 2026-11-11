use recon_core::error::{
    ActionFailure, ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};
use recon_core::process::Pid;
use recon_native_access_macos::identity::Incarnation;
use recon_native_access_macos::inject;
use recon_window_server_macos::inventory;
use recon_window_server_macos::window::{Window, WindowId, select_window};

use crate::failure::action;

#[derive(Clone, Debug)]
pub struct Target {
    incarnation: Incarnation,
    window: Window,
}

impl Target {
    pub fn bind(pid: Pid, window: WindowId, mechanism: ActionMechanism) -> Result<Self, Failure> {
        let incarnation =
            inject::identify(pid.0 as i32).map(Incarnation::from).map_err(|error| {
                action(
                    ErrorCode::StaleTarget,
                    format!("process {pid} changed before it could be bound: {error}"),
                    ActionFailure {
                        stage: ActionStage::Resolve,
                        dispatch: DispatchState::None,
                        mechanism,
                        pid: pid.0,
                        window_id: window.0,
                        release: ReleaseState::NotNeeded,
                        key_window: None,
                    },
                )
            })?;
        let selected = select_window(inventory::windows_owned_by(pid)?, window, pid)?;
        Ok(Self { incarnation, window: selected })
    }

    pub fn incarnation(&self) -> Incarnation {
        self.incarnation
    }

    pub fn pid(&self) -> Pid {
        Pid(self.incarnation.pid as u32)
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn revalidate(
        &self,
        mechanism: ActionMechanism,
        dispatch: DispatchState,
        release: ReleaseState,
    ) -> Result<Window, Failure> {
        let stale = |detail: String| {
            action(
                ErrorCode::StaleTarget,
                detail,
                ActionFailure {
                    stage: ActionStage::Dispatch,
                    dispatch,
                    mechanism,
                    pid: self.pid().0,
                    window_id: self.window.id.0,
                    release: if release == ReleaseState::Failed {
                        ReleaseState::SkippedTargetChanged
                    } else {
                        release
                    },
                    key_window: None,
                },
            )
        };
        let current =
            inject::identify(self.incarnation.pid).map(Incarnation::from).map_err(|error| {
                stale(format!("the target process is no longer the bound incarnation: {error}"))
            })?;
        if current != self.incarnation {
            return Err(stale(format!("process {} changed incarnation", self.incarnation.pid)));
        }
        let window = inventory::windows_owned_by(self.pid())
            .map_err(|error| stale(error.message.clone()))?
            .into_iter()
            .find(|window| window.id == self.window.id && window.owner_pid == self.pid())
            .ok_or_else(|| {
                stale(format!("window {} is no longer owned by the bound process", self.window.id))
            })?;
        Ok(window)
    }

    pub fn validate_point(
        window: &Window,
        point: super::request::Point,
        mechanism: ActionMechanism,
    ) -> Result<(), Failure> {
        let bounds = window.bounds;
        let valid = point.x.is_finite()
            && point.y.is_finite()
            && point.x >= 0.0
            && point.y >= 0.0
            && point.x < bounds.width
            && point.y < bounds.height;
        if valid {
            return Ok(());
        }
        Err(action(
            ErrorCode::ActionUnavailable,
            format!("point ({}, {}) lies outside window {} bounds", point.x, point.y, window.id),
            ActionFailure {
                stage: ActionStage::Resolve,
                dispatch: DispatchState::None,
                mechanism,
                pid: window.owner_pid.0,
                window_id: window.id.0,
                release: ReleaseState::NotNeeded,
                key_window: None,
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposal0007_exit_invalidates_identity_and_skips_an_owed_release() {
        let mut child = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("spawn process fixture");
        let pid = Pid(child.id());
        let incarnation = inject::identify(pid.0 as i32).map(Incarnation::from).unwrap();
        let target = Target {
            incarnation,
            window: Window {
                id: WindowId(123),
                owner_pid: pid,
                title: None,
                layer: 0,
                bounds: recon_window_server_macos::window::Bounds {
                    x: 0.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                on_screen: true,
                alpha: 1.0,
            },
        };
        child.kill().unwrap();
        child.wait().unwrap();

        let failure = target
            .revalidate(ActionMechanism::MouseEvents, DispatchState::Partial, ReleaseState::Failed)
            .unwrap_err();

        assert_eq!(failure.code, ErrorCode::StaleTarget);
        let details = failure.details.expect("details");
        assert_eq!(details["dispatch"], "partial");
        assert_eq!(details["release"], "skipped-target-changed");
    }
}
