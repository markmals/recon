use std::any::TypeId;

use clap::builder::ValueParser;
use serde_json::{Value, json};

#[derive(Clone, Debug)]
pub(super) struct OutputPath(pub(super) String);

impl OutputPath {
    pub(super) fn parser() -> ValueParser {
        ValueParser::new(|text: &str| {
            if text.starts_with('-') {
                Err("output path must not start with a hyphen".to_owned())
            } else {
                Ok(Self(text.to_owned()))
            }
        })
    }
}

pub(super) fn constraints(parser: &ValueParser) -> Option<Value> {
    (parser.type_id() == TypeId::of::<OutputPath>()).then(|| json!({"pattern":"^(?:[^-]|$)"}))
}
