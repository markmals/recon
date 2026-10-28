//! The `recon ui` command grammar.

use std::path::PathBuf;

use crate::application::Pid;
use crate::ui::window::WindowId;

/// The application a `recon ui` command observes, and the instance `--pid` chose, if any.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selection {
    pub target: String,
    pub pid: Option<Pid>,
}

/// One `recon ui` command. Without a `window`, `ax tree` reads the whole application and
/// `screenshot` captures the application's main window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiRequest {
    Windows {
        selection: Selection,
    },
    AxTree {
        selection: Selection,
        window: Option<WindowId>,
        depth: Option<usize>,
        no_menu_bar: bool,
    },
    Screenshot {
        selection: Selection,
        window: Option<WindowId>,
        out: PathBuf,
    },
}
