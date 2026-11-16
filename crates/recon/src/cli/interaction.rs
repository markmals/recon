use std::any::TypeId;

use clap::ArgMatches;
use clap::builder::ValueParser;
use serde_json::{Value, json};

use recon_core::error::Failure;
use recon_input_macos::chord::{Chord, Modifiers};
use recon_input_macos::request::MouseButton;
use recon_input_macos::request::{ClickMethod, ElementSelector, Location, Point, ScrollUnit};

use super::native::FilterText;
use super::{choice, parse_error, selection, session, window};
use crate::ui::request::UiRequest;

#[derive(Clone, Copy, Debug)]
pub(super) struct Coordinate(pub f64);

#[derive(Clone, Copy, Debug)]
pub(super) struct SignedDelta(pub i32);

pub(super) fn coordinate_parser() -> ValueParser {
    ValueParser::new(|text: &str| {
        text.parse::<f64>()
            .ok()
            .filter(|coordinate| coordinate.is_finite())
            .map(Coordinate)
            .ok_or_else(|| "expected a finite logical point coordinate".to_owned())
    })
}

pub(super) fn delta_parser() -> ValueParser {
    ValueParser::new(|text: &str| {
        let digits = text.strip_prefix('-').unwrap_or(text);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("expected a signed ASCII integer".to_owned());
        }
        text.parse::<i32>()
            .map(SignedDelta)
            .map_err(|_| "integer outside accepted range".to_owned())
    })
}

pub(super) fn modifiers_parser() -> ValueParser {
    ValueParser::new(|text: &str| Modifiers::parse_click(text))
}

pub(super) fn chord_parser() -> ValueParser {
    ValueParser::new(|text: &str| Chord::parse(text))
}

pub(super) fn constraints(parser: &ValueParser) -> Option<Value> {
    if parser.type_id() == TypeId::of::<Coordinate>() {
        Some(json!({"type":"number","finite":true}))
    } else if parser.type_id() == TypeId::of::<SignedDelta>() {
        Some(json!({"type":"integer","minimum":-10000,"maximum":10000}))
    } else if parser.type_id() == TypeId::of::<Modifiers>() {
        Some(json!({
            "type":"string",
            "accepted":["cmd","ctrl","alt","shift"],
            "separator":",",
            "unique":true
        }))
    } else if parser.type_id() == TypeId::of::<Chord>() {
        Some(json!({"type":"string"}))
    } else {
        None
    }
}

pub(super) fn click(
    path: &'static [&'static str],
    matches: &ArgMatches,
) -> Result<UiRequest, Failure> {
    let location = location(path, matches)?;
    let method = choice(matches, "method", ClickMethod::parse);
    let button = choice(matches, "button", MouseButton::parse);
    let count = matches.get_one::<super::Decimal<1, 2>>("count").expect("count default").0 as u8;
    let modifiers = matches.get_one::<Modifiers>("modifiers").copied().unwrap_or_default();
    if method == ClickMethod::Ax
        && (!matches!(location, Location::Element(_))
            || button != MouseButton::Left
            || count != 1
            || !modifiers.is_empty())
    {
        return Err(parse_error::usage(
            path,
            "--method ax requires an element, left button, count 1, and no modifiers".to_owned(),
            Some("--method"),
            Some("ax"),
            "an eligible plain element click",
            None,
        ));
    }
    Ok(UiRequest::Click {
        selection: selection(matches),
        session: session(matches),
        window: window(matches).expect("required window"),
        location,
        method,
        button,
        count,
        modifiers,
    })
}

pub(super) fn scroll(
    path: &'static [&'static str],
    matches: &ArgMatches,
) -> Result<UiRequest, Failure> {
    let location = location(path, matches)?;
    let unit = choice(matches, "unit", ScrollUnit::parse);
    let dx = matches.get_one::<SignedDelta>("dx").map_or(0, |delta| delta.0);
    let dy = matches.get_one::<SignedDelta>("dy").map_or(0, |delta| delta.0);
    if dx == 0 && dy == 0 {
        return Err(parse_error::usage(
            path,
            "ui scroll requires at least one non-zero delta".to_owned(),
            None,
            None,
            "a non-zero --dx or --dy",
            None,
        ));
    }
    let maximum = unit.maximum_magnitude();
    if dx.unsigned_abs() > maximum as u32 || dy.unsigned_abs() > maximum as u32 {
        return Err(parse_error::usage(
            path,
            format!("{unit:?} scroll magnitudes must not exceed {maximum}"),
            None,
            None,
            &format!("--dx and --dy between -{maximum} and {maximum}"),
            None,
        ));
    }
    Ok(UiRequest::Scroll {
        selection: selection(matches),
        session: session(matches),
        window: window(matches).expect("required window"),
        location,
        dx,
        dy,
        unit,
    })
}

pub(super) fn key(matches: &ArgMatches) -> UiRequest {
    UiRequest::Key {
        selection: selection(matches),
        session: session(matches),
        window: window(matches).expect("required window"),
        chord: matches.get_one::<Chord>("chord").expect("required chord").clone(),
    }
}

fn location(path: &'static [&'static str], matches: &ArgMatches) -> Result<Location, Failure> {
    let selector = ElementSelector {
        identifier: matches.get_one::<FilterText>("identifier").map(|text| text.0.clone()),
        role: matches.get_one::<FilterText>("role").map(|text| text.0.clone()),
        title: matches.get_one::<FilterText>("title").map(|text| text.0.clone()),
    };
    let point = matches.get_one::<Coordinate>("x").zip(matches.get_one::<Coordinate>("y"));
    match (selector.is_empty(), point) {
        (false, None) => Ok(Location::Element(selector)),
        (true, Some((x, y))) => Ok(Location::Point(Point { x: x.0, y: y.0 })),
        (false, Some(_)) => Err(parse_error::usage(
            path,
            "element filters and coordinates are alternative location forms".to_owned(),
            None,
            None,
            "element filters or a complete --x/--y pair, not both",
            None,
        )),
        _ => Err(parse_error::usage(
            path,
            "an action location requires element filters or a complete --x/--y pair".to_owned(),
            None,
            None,
            "at least one of --identifier, --role, --title, or both --x and --y",
            None,
        )),
    }
}
