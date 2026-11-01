//! `recon ui ax tree`: the accessibility representation of an application or one of its windows.

use recon_accessibility_macos::element::{self, Accessibility, Element};
use recon_accessibility_macos::tree::{TreeOptions, read_tree};
use recon_accessibility_macos::window_element;
use recon_applications_macos::selection::Selection;
use recon_core::error::Failure;
use recon_native_access_macos::identity::SessionId;
use recon_privacy_macos::permission::Permission;
use recon_privacy_macos::permissions;
use recon_window_server_macos::inventory::windows_owned_by;
use recon_window_server_macos::window::{WindowId, select_window};

use super::bound;
use super::target::{observed, with_instance};
use crate::ui::response::TreeResponse;

pub(super) fn tree(
    selection: &Selection,
    session: Option<&SessionId>,
    window: Option<WindowId>,
    depth: Option<usize>,
    no_menu_bar: bool,
) -> Result<String, Failure> {
    with_instance(selection, session, |application, pid| {
        let summary = bound(application, pid, session)?;
        if let Some(id) = window {
            select_window(windows_owned_by(pid)?, id, pid)?;
        }
        permissions::require(&[Permission::Accessibility])?;

        element::bound_messaging();
        let application_element = element::application_element(pid)?;
        let excluded_root_child = if no_menu_bar && depth != Some(0) {
            element::menu_bar(&application_element, pid)?
        } else {
            None
        };
        let root = match window {
            Some(id) => window_element::window_element(&application_element, pid, id)?,
            None => application_element,
        };

        let tree = read_tree(
            &mut Accessibility::new(),
            Element::new(root),
            TreeOptions { depth, excluded_root_child },
        );
        let response = TreeResponse::new(
            observed(application, pid),
            window,
            tree.truncated,
            tree.narrowed,
            tree.root,
        );
        let response = match summary {
            Some(summary) => response.with_session(summary),
            None => response,
        };
        crate::output::encode(&response)
    })
}
