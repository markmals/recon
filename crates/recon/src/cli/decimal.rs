use std::any::TypeId;

use clap::builder::ValueParser;
use serde::Serialize;

#[derive(Clone, Copy, Debug)]
pub(super) struct Decimal<const MINIMUM: u32, const MAXIMUM: u32 = { u32::MAX }>(pub(super) u32);

impl<const MINIMUM: u32, const MAXIMUM: u32> Decimal<MINIMUM, MAXIMUM> {
    pub(super) fn parser() -> ValueParser {
        ValueParser::new(|text: &str| {
            if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err("expected ASCII decimal digits".to_owned());
            }
            text.parse::<u32>()
                .ok()
                .filter(|number| (MINIMUM..=MAXIMUM).contains(number))
                .map(Self)
                .ok_or_else(|| "number outside accepted range".to_owned())
        })
    }

    fn constraints() -> Constraints {
        Constraints { pattern: "^[0-9]+$", minimum: MINIMUM, maximum: MAXIMUM }
    }
}

#[derive(Serialize)]
pub(super) struct Constraints {
    pub(super) pattern: &'static str,
    pub(super) minimum: u32,
    pub(super) maximum: u32,
}

/// The ranges Recon publishes. A native traversal's depth bound is the same number as the
/// accessibility tree's, so both spellings resolve to one parser type here.
pub(super) fn constraints(parser: &ValueParser) -> Option<Constraints> {
    if parser.type_id() == TypeId::of::<Decimal<0>>() {
        Some(Decimal::<0>::constraints())
    } else if parser.type_id() == TypeId::of::<Decimal<1>>() {
        Some(Decimal::<1>::constraints())
    } else if parser.type_id() == TypeId::of::<Decimal<0, { crate::ui::tree::MAX_DEPTH as u32 }>>()
    {
        Some(Decimal::<0, { crate::ui::tree::MAX_DEPTH as u32 }>::constraints())
    } else if parser.type_id()
        == TypeId::of::<Decimal<1, { crate::ui::request::MAX_NATIVE_LIMIT as u32 }>>()
    {
        Some(Decimal::<1, { crate::ui::request::MAX_NATIVE_LIMIT as u32 }>::constraints())
    } else {
        None
    }
}
