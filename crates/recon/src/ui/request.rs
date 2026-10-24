use std::path::PathBuf;

use crate::request::USAGE;
use crate::ui::window::WindowId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Selection {
    pub target: String,
    pub pid: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiRequest {
    Windows { selection: Selection },
    Tree { selection: Selection, window: Option<WindowId> },
    Screenshot { selection: Selection, window: Option<WindowId>, out: PathBuf },
}

#[derive(Default)]
struct Flags {
    pid: Option<u32>,
    window: Option<WindowId>,
    out: Option<PathBuf>,
}

/// Parses the arguments that follow `recon ui`.
pub fn parse_ui_arguments(arguments: &[String]) -> Result<UiRequest, String> {
    let mut words = arguments.iter().map(String::as_str);
    let command = words.next().ok_or_else(usage)?;
    let (target, flags) = parse_target_and_flags(words)?;
    let selection = Selection { target: target.to_owned(), pid: flags.pid };

    match command {
        "windows" if flags.window.is_none() && flags.out.is_none() => {
            Ok(UiRequest::Windows { selection })
        }
        "tree" if flags.out.is_none() => Ok(UiRequest::Tree { selection, window: flags.window }),
        "screenshot" => match flags.out {
            Some(out) => Ok(UiRequest::Screenshot { selection, window: flags.window, out }),
            None => Err(usage()),
        },
        _ => Err(usage()),
    }
}

fn parse_target_and_flags<'a>(
    mut words: impl Iterator<Item = &'a str>,
) -> Result<(&'a str, Flags), String> {
    let mut target = None;
    let mut flags = Flags::default();
    while let Some(word) = words.next() {
        match word {
            "--pid" => set(&mut flags.pid, positive_integer(value_of(&mut words)?)?)?,
            "--window" => set(&mut flags.window, window_id(value_of(&mut words)?)?)?,
            "--out" => set(&mut flags.out, PathBuf::from(value_of(&mut words)?))?,
            _ if is_flag(word) => return Err(usage()),
            _ => set(&mut target, word)?,
        }
    }
    Ok((target.ok_or_else(usage)?, flags))
}

fn value_of<'a>(words: &mut impl Iterator<Item = &'a str>) -> Result<&'a str, String> {
    words.next().filter(|value| !is_flag(value)).ok_or_else(usage)
}

fn set<T>(flag: &mut Option<T>, value: T) -> Result<(), String> {
    if flag.is_some() {
        return Err(usage());
    }
    *flag = Some(value);
    Ok(())
}

fn window_id(value: &str) -> Result<WindowId, String> {
    digits(value).map(WindowId).ok_or_else(usage)
}

fn positive_integer(value: &str) -> Result<u32, String> {
    digits(value).filter(|number| *number > 0).ok_or_else(usage)
}

/// Accepts only plain decimal digits, so `+12`, ` 12`, and `12.0` are not numbers here.
fn digits(value: &str) -> Option<u32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn is_flag(word: &str) -> bool {
    word.starts_with("--")
}

fn usage() -> String {
    USAGE.to_owned()
}
