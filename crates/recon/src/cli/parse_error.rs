use clap::error::{ContextKind, ContextValue, ErrorKind};
use serde_json::{Value, json};

use super::registry::CommandSpec;
use crate::error::{ErrorCode, Failure};

fn invalid(
    message: impl Into<String>,
    path: &[&str],
    argument: Option<&str>,
    value: Option<&str>,
    expected: &str,
    accepted: Option<Vec<String>>,
) -> Failure {
    let failure = Failure::new(
        ErrorCode::InvalidArgument,
        message,
        json!({"argument":argument,"value":value,"expected":expected,"accepted":accepted}),
    );
    if path.is_empty() { failure } else { failure.with_command(path) }
}

fn text(error: &clap::Error, kind: ContextKind) -> Option<String> {
    match error.get(kind) {
        Some(ContextValue::String(value)) => Some(value.clone()),
        Some(ContextValue::Strings(values)) => values.first().cloned(),
        _ => None,
    }
}

pub(super) fn convert(
    error: clap::Error,
    words: &[String],
    entry: Option<&CommandSpec>,
    path: &[&str],
    registry: &[CommandSpec],
) -> Failure {
    let invocation =
        if path.is_empty() { "recon".into() } else { format!("recon {}", path.join(" ")) };
    if entry.is_none() {
        let accepted = registry
            .iter()
            .filter(|entry| entry.path.starts_with(path))
            .filter_map(|entry| entry.path.get(path.len()))
            .map(|word| (*word).to_owned())
            .fold(Vec::new(), |mut words, word| {
                if !words.contains(&word) {
                    words.push(word);
                }
                words
            });
        let value = if error.kind() == ErrorKind::MissingSubcommand {
            None
        } else {
            text(&error, ContextKind::InvalidSubcommand)
                .or_else(|| text(&error, ContextKind::InvalidArg))
        };
        let noun = if path.is_empty() { "a command" } else { "an operation" };
        let expected = format!("{noun} of {invocation}");
        return invalid(
            format!("{invocation} requires {noun}"),
            &[],
            None,
            value.as_deref(),
            &expected,
            Some(accepted),
        );
    }
    let entry = entry.expect("entry checked");
    let mut parser = entry.parser();
    parser.build();
    let raw_argument = text(&error, ContextKind::InvalidArg).unwrap_or_default();
    let argument_name = raw_argument.split_whitespace().next().unwrap_or_default();
    let declaration = parser.get_arguments().find(|argument| {
        argument_name == name(argument)
            || argument
                .get_value_names()
                .is_some_and(|names| names.iter().any(|name| raw_argument.contains(name.as_str())))
    });
    let value = text(&error, ContextKind::InvalidValue);
    if matches!(error.kind(), ErrorKind::MissingRequiredArgument) {
        let missing = declaration.or_else(|| {
            parser.get_arguments().find(|argument| {
                argument.is_required_set() && !words.iter().any(|word| *word == name(argument))
            })
        });
        if let Some(argument) = missing {
            let name = name(argument);
            return invalid(
                format!("{invocation} requires {name}"),
                path,
                Some(&name),
                None,
                &description(argument),
                None,
            );
        }
    }
    if matches!(error.kind(), ErrorKind::ArgumentConflict) {
        let name = declaration.map(name).unwrap_or_else(|| argument_name.to_owned());
        let repeated = words
            .iter()
            .enumerate()
            .filter(|(_, word)| **word == name)
            .nth(1)
            .and_then(|(index, _)| words.get(index + 1))
            .map(String::as_str);
        return invalid(
            format!("{name} was given more than once"),
            path,
            Some(&name),
            repeated,
            &format!("{name} at most once"),
            None,
        );
    }
    if let Some(argument) = declaration {
        let name = name(argument);
        let expected = description(argument);
        let value = value.or_else(|| {
            words
                .iter()
                .position(|word| *word == name)
                .and_then(|index| words.get(index + 1))
                .filter(|word| !word.starts_with('-'))
                .cloned()
        });
        return invalid(
            match &value {
                Some(value) => format!("{name} expects {expected}; got {value:?}"),
                None => format!("{name} expects {expected}"),
            },
            path,
            Some(&name),
            value.as_deref(),
            &expected,
            None,
        );
    }
    if argument_name.starts_with('-') {
        let accepted =
            parser.get_arguments().filter(|argument| !argument.is_positional()).map(name).collect();
        return invalid(
            format!("{invocation} does not take {argument_name}"),
            path,
            Some(argument_name),
            None,
            &format!("an option {invocation} accepts"),
            Some(accepted),
        );
    }
    let value = if raw_argument.is_empty() { Value::Null } else { json!(raw_argument) };
    invalid(
        format!("{invocation} takes one target; {raw_argument:?} is extra"),
        path,
        None,
        value.as_str(),
        "exactly one target",
        None,
    )
}

fn name(argument: &clap::Arg) -> String {
    argument.get_long().map_or_else(|| argument.get_id().to_string(), |long| format!("--{long}"))
}

fn description(argument: &clap::Arg) -> String {
    let description = argument.get_help().map(ToString::to_string).unwrap_or_default();
    let mut letters = description.chars();
    match letters.next() {
        Some(first) => first.to_lowercase().chain(letters).collect(),
        None => description,
    }
}
