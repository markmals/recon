//! The `recon ui` command grammar.
//!
//! `ui` is the interface domain: `ax` queries the accessibility representation, `view` the native
//! visual one, `session` the inspection-access relationship, and `windows`/`screenshot`/`correlate`
//! are shared operations. A session is never implied: an external command runs exactly as it did
//! before when `--session` is absent, and a native command cannot run without one.

use std::path::PathBuf;

use recon_applications_macos::selection::Selection;
use recon_native_access_macos::identity::{AccessMode, AccessProfile, SessionId};
use recon_native_access_macos::request::{
    FieldSet, Filters, HostPreparation, NativeScope, NodeHandle, TraversalKind,
};
use recon_window_server_macos::window::WindowId;

/// One `recon ui` command. Without a `window`, `ax tree` reads the whole application and
/// `screenshot` captures the application's main window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiRequest {
    Windows {
        selection: Selection,
        session: Option<SessionId>,
    },
    AxTree {
        selection: Selection,
        session: Option<SessionId>,
        window: Option<WindowId>,
        depth: Option<usize>,
        no_menu_bar: bool,
    },
    Screenshot {
        selection: Selection,
        session: Option<SessionId>,
        window: Option<WindowId>,
        out: PathBuf,
    },
    Session(SessionRequest),
    View(ViewRequest),
    /// Gather native and external accessibility evidence for the same selected window and report
    /// candidate relationships between them.
    Correlate {
        selection: Selection,
        session: SessionId,
        window: WindowId,
        depth: usize,
        limit: usize,
    },
}

/// One `recon ui session` command: the inspection-access lifecycle, which never closes a window or
/// quits an application.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionRequest {
    /// Report access eligibility and unresolved prerequisites without loading code or prompting.
    Preflight { selection: Selection },
    /// Explicit, opt-in research-host preparation.
    PrepareHost { action: HostPreparation },
    /// Establish one explicitly requested session.
    Open {
        selection: Selection,
        mode: AccessMode,
        profile: AccessProfile,
        allow_authorization: bool,
    },
    /// Close and invalidate one session by its identifier.
    Close { session: SessionId },
}

/// One `recon ui view` command: native visual inspection inside an open session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ViewRequest {
    Tree {
        selection: Selection,
        session: SessionId,
        scope: NativeScope,
        kind: TraversalKind,
        depth: usize,
        fields: FieldSet,
        limit: usize,
    },
    Node {
        selection: Selection,
        session: SessionId,
        handle: NodeHandle,
        fields: FieldSet,
    },
    Find {
        selection: Selection,
        session: SessionId,
        scope: NativeScope,
        kind: TraversalKind,
        filters: Filters,
        depth: usize,
        fields: FieldSet,
        limit: usize,
    },
    Fonts {
        selection: Selection,
        session: SessionId,
        scope: NativeScope,
        depth: usize,
        limit: usize,
    },
}
