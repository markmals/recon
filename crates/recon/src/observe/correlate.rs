//! `recon ui correlate`: candidate relationships between one native observation and one
//! accessibility observation of the same window.
//!
//! Neither source owns the other. A candidate records that the two reported matching evidence;
//! it never claims a native object and an accessibility element are one object.

use recon_applications_macos::selection::Selection;
use recon_core::error::Failure;
use recon_native_access_macos::SessionId;
use recon_native_access_macos::request::{FieldSet, NativeScope, TraversalKind};
use recon_window_server_macos::window::WindowId;
use serde_json::{Value, json};

use super::target::with_instance;
use crate::correlate::{ElementReference, MatchBasis, NativeReference};
use crate::ui::request::ViewRequest;

pub(super) fn correlate(
    selection: &Selection,
    session: &SessionId,
    window: WindowId,
    depth: usize,
    limit: usize,
) -> Result<String, Failure> {
    let native = with_instance(selection, Some(session), |application, pid| {
        let request = ViewRequest::Tree {
            selection: selection.clone(),
            session: session.clone(),
            scope: NativeScope { window: Some(window), root: None },
            kind: TraversalKind::View,
            depth,
            fields: FieldSet::default(),
            limit,
        };
        super::session::view(application, pid, session, &request)
    })?;
    let accessibility =
        super::tree::tree(selection, Some(session), Some(window), Some(depth), false)?;
    let accessibility: Value = serde_json::from_str(&accessibility)
        .map_err(|error| Failure::internal(format!("cannot decode AX observation: {error}")))?;
    let correlation = crate::correlate::correlate(&native, &accessibility);
    let candidates = correlation
        .candidates
        .iter()
        .map(|candidate| {
            json!({
                "native": native_reference(&candidate.native),
                "accessibility": element_reference(&candidate.accessibility),
                "basis": match candidate.basis {
                    MatchBasis::AccessibilityIdentifier => "accessibility-identifier",
                    MatchBasis::RoleAndBounds => "role-and-bounds",
                },
                "ambiguous": candidate.ambiguous,
            })
        })
        .collect::<Vec<_>>();
    crate::output::encode(&json!({
        "native": native,
        "accessibility": accessibility,
        "candidates": candidates,
        "unmatchedNative": correlation.unmatched_native.iter().map(native_reference).collect::<Vec<_>>(),
        "unmatchedAccessibility": correlation.unmatched_accessibility.iter().map(element_reference).collect::<Vec<_>>(),
    }))
}

fn native_reference(reference: &NativeReference) -> Value {
    json!({
        "handle": reference.handle,
        "kind": reference.kind,
        "class": reference.class,
    })
}

fn element_reference(reference: &ElementReference) -> Value {
    json!({
        "path": reference.path,
        "role": reference.role,
        "identifier": reference.identifier,
    })
}
