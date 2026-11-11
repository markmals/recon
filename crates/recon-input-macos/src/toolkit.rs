use std::path::Path;

use recon_core::error::{
    ActionMechanism, ActionStage, DispatchState, ErrorCode, Failure, ReleaseState,
};

use crate::failure::action;
use crate::target::Target;

pub use recon_applications_macos::toolkit::Toolkit;

pub fn read(target: &Target, executable: &Path) -> Result<Toolkit, Failure> {
    recon_applications_macos::toolkit::read(executable).map_err(|detail| {
        action(
            ErrorCode::ActionUnavailable,
            detail,
            crate::failure::details(
                target,
                ActionMechanism::KeyEvents,
                ActionStage::Resolve,
                DispatchState::None,
                ReleaseState::NotNeeded,
            ),
        )
    })
}
