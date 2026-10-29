//! The value parsers the native inspection options use, and the constraints the registry
//! publishes for them.
//!
//! Every one of these values names something that outlives the command line: a session, a live
//! object, a set of observations the helper must actually make. Rejecting a malformed spelling
//! here is what keeps a usage mistake from arriving at the target as a lookup that fails later.

use std::any::TypeId;

use clap::builder::ValueParser;
use serde_json::{Value, json};

use crate::inspection::SessionId;
use crate::ui::request::{FieldGroup, FieldSet, HANDLE_DIGITS, NodeHandle};

/// Digits in a session identifier's published spelling, as [`SessionId::parse`] accepts it.
const SESSION_DIGITS: usize = 32;

/// The longest exact filter text a request may carry, in Unicode scalars. It matches the cap on
/// returned strings, so a filter cannot be longer than the value it could ever match.
const MAX_FILTER_SCALARS: usize = 4_096;

/// One exact `--class` or `--identifier` filter: non-empty and bounded.
#[derive(Clone, Debug)]
pub(super) struct FilterText(pub(super) String);

pub(super) fn session_parser() -> ValueParser {
    ValueParser::new(|text: &str| {
        SessionId::parse(text).map_err(|error| error.message().to_owned())
    })
}

pub(super) fn handle_parser() -> ValueParser {
    ValueParser::new(NodeHandle::parse)
}

pub(super) fn fields_parser() -> ValueParser {
    ValueParser::new(FieldSet::parse)
}

pub(super) fn filter_parser() -> ValueParser {
    ValueParser::new(|text: &str| {
        let scalars = text.chars().count();
        if scalars == 0 {
            Err("expected text to match exactly".to_owned())
        } else if scalars > MAX_FILTER_SCALARS {
            Err(format!("expected at most {MAX_FILTER_SCALARS} characters"))
        } else {
            Ok(FilterText(text.to_owned()))
        }
    })
}

/// A regular expression that accepts exactly the comma-separated field-group lists, so a caller
/// can check a `--fields` value without running Recon. Repeats are a usage error that the pattern
/// cannot express, which is why `unique` accompanies it.
fn field_pattern() -> String {
    let groups = FieldGroup::ALL.join("|");
    format!("^({groups})(,({groups}))*$")
}

pub(super) fn constraints(parser: &ValueParser) -> Option<Value> {
    if parser.type_id() == TypeId::of::<SessionId>() {
        Some(json!({"type":"string","pattern":format!("^[0-9a-f]{{{SESSION_DIGITS}}}$")}))
    } else if parser.type_id() == TypeId::of::<NodeHandle>() {
        Some(json!({"type":"string","pattern":format!("^[0-9a-f]{{{HANDLE_DIGITS}}}$")}))
    } else if parser.type_id() == TypeId::of::<FieldSet>() {
        Some(json!({
            "type":"string",
            "pattern":field_pattern(),
            "accepted":FieldGroup::ALL,
            "separator":FieldSet::SEPARATOR.to_string(),
            "unique":true,
        }))
    } else if parser.type_id() == TypeId::of::<FilterText>() {
        Some(json!({"type":"string","minLength":1,"maxLength":MAX_FILTER_SCALARS}))
    } else {
        None
    }
}
