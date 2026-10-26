use clap::error::{ContextKind, ContextValue, ErrorKind};
use clap::{Arg, Command};
use serde_json::json;

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

fn text(error: &clap::Error, kind: ContextKind) -> Option<&str> {
    match error.get(kind) {
        Some(ContextValue::String(value)) => Some(value),
        Some(ContextValue::Strings(values)) => values.first().map(String::as_str),
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
    let Some(entry) = entry else {
        return command_error(&error, words, path, registry, &invocation);
    };
    let mut parser = entry.parser();
    parser.build();
    if error.kind() == ErrorKind::UnknownArgument {
        let argument = text(&error, ContextKind::InvalidArg).unwrap_or_default();
        return unknown_argument(unknown_token(argument, words), words, path, &invocation, &parser);
    }
    argument_error(&error, words, path, &invocation, &parser)
}

fn unknown_token<'a>(argument: &'a str, words: &'a [String]) -> &'a str {
    if argument.starts_with('-') && !argument.starts_with("--") {
        words.iter().find(|word| word.starts_with(argument)).map_or(argument, String::as_str)
    } else {
        argument
    }
}

fn command_error(
    error: &clap::Error,
    words: &[String],
    path: &[&str],
    registry: &[CommandSpec],
    invocation: &str,
) -> Failure {
    if error.kind() == ErrorKind::UnknownArgument {
        let argument =
            unknown_token(text(error, ContextKind::InvalidArg).unwrap_or_default(), words);
        if path.is_empty() && argument == super::VERSION_FLAG {
            return invalid(
                "--version must be used alone",
                &[],
                Some(argument),
                words.get(1).map(String::as_str),
                "--version with no other arguments",
                Some(Vec::new()),
            );
        }
        let mut accepted = super::HELP_FLAGS.into_iter().map(str::to_owned).collect::<Vec<_>>();
        if path.is_empty() {
            accepted.push(super::VERSION_FLAG.to_owned());
        }
        return invalid(
            format!("{invocation} does not take {argument}"),
            &[],
            Some(argument),
            None,
            "an orientation flag",
            Some(accepted),
        );
    }
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
        text(error, ContextKind::InvalidSubcommand).or_else(|| text(error, ContextKind::InvalidArg))
    };
    let noun = if path.is_empty() { "a command" } else { "an operation" };
    invalid(
        format!("{invocation} requires {noun}"),
        &[],
        None,
        value,
        &format!("{noun} of {invocation}"),
        Some(accepted),
    )
}

fn unknown_argument(
    argument: &str,
    words: &[String],
    path: &[&str],
    invocation: &str,
    parser: &Command,
) -> Failure {
    if argument.starts_with('-') && !after_separator(argument, words, parser) {
        let accepted =
            parser.get_arguments().filter(|argument| !argument.is_positional()).map(name).collect();
        return invalid(
            format!("{invocation} does not take {argument}"),
            path,
            Some(argument),
            None,
            &format!("an option {invocation} accepts"),
            Some(accepted),
        );
    }
    if parser.get_positionals().next().is_none() {
        return invalid(
            format!("{invocation} takes no positional arguments; {argument:?} is extra"),
            path,
            None,
            Some(argument),
            "no positional arguments",
            Some(Vec::new()),
        );
    }
    invalid(
        format!("{invocation} takes one target; {argument:?} is extra"),
        path,
        None,
        Some(argument),
        "exactly one target",
        None,
    )
}

fn incompatible_options<'a>(
    parser: &'a Command,
    invalid: &Arg,
    words: &[String],
) -> Option<(&'a Arg, &'a Arg)> {
    // Clap lists declared conflicts in one direction; input order can name either side.
    parser.get_arguments().filter(|argument| is_supplied(argument, words)).find_map(|argument| {
        parser
            .get_arg_conflicts_with(argument)
            .into_iter()
            .find(|conflict| {
                is_supplied(conflict, words)
                    && (argument.get_id() == invalid.get_id()
                        || conflict.get_id() == invalid.get_id())
            })
            .map(|conflict| (argument, conflict))
    })
}

fn is_supplied(argument: &Arg, words: &[String]) -> bool {
    argument.get_long().is_some_and(|name| {
        words.iter().take_while(|word| *word != "--").any(|word| {
            word.strip_prefix("--").is_some_and(|word| word.split('=').next() == Some(name))
        })
    })
}

fn after_separator(argument: &str, words: &[String], parser: &Command) -> bool {
    let Some(separator) = words.iter().position(|word| word == "--") else {
        return false;
    };
    if !words[separator + 1..].iter().any(|word| word == argument) {
        return false;
    }
    // A recognized option before `--` is valid; the same spelling after it is a positional.
    argument.strip_prefix("--").is_some_and(|long| {
        parser.get_arguments().any(|declared| declared.get_long() == Some(long))
    }) || !words[..separator].iter().any(|word| word == argument)
}

fn argument_error(
    error: &clap::Error,
    words: &[String],
    path: &[&str],
    invocation: &str,
    parser: &Command,
) -> Failure {
    let raw_argument = text(error, ContextKind::InvalidArg).unwrap_or_default();
    let argument_name = raw_argument.split_whitespace().next().unwrap_or_default();
    let positional = raw_argument.trim_end_matches("...");
    let positional = positional
        .strip_prefix('<')
        .and_then(|name| name.strip_suffix('>'))
        .or_else(|| positional.strip_prefix('[').and_then(|name| name.strip_suffix(']')));
    let declaration = parser.get_arguments().find(|argument| {
        argument.get_long().is_some_and(|long| argument_name.strip_prefix("--") == Some(long))
            || argument.is_positional()
                && argument
                    .get_value_names()
                    .is_some_and(|names| names.iter().any(|name| Some(name.as_str()) == positional))
    });
    if error.kind() == ErrorKind::MissingRequiredArgument
        && let Some(argument) = declaration
            .or_else(|| parser.get_arguments().find(|argument| argument.is_required_set()))
    {
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
    if error.kind() == ErrorKind::ArgumentConflict {
        if let Some(argument) = declaration
            && let Some((argument, conflict)) = incompatible_options(parser, argument, words)
        {
            let argument_name = name(argument);
            let other_name = name(conflict);
            return invalid(
                format!(
                    "{argument_name} cannot be combined with {other_name}; {}; {}",
                    description(argument),
                    description(conflict),
                ),
                path,
                Some(&argument_name),
                None,
                &format!("{argument_name} without {other_name}"),
                None,
            );
        }
        let name = declaration.map(name).unwrap_or_else(|| argument_name.to_owned());
        return invalid(
            format!("{name} was given more than once"),
            path,
            Some(&name),
            supplied_value(words, &name, 1),
            &format!("{name} at most once"),
            None,
        );
    }
    let Some(argument) = declaration else {
        return invalid(
            format!("{invocation} has invalid arguments"),
            path,
            None,
            None,
            &format!("valid arguments for {invocation}"),
            None,
        );
    };
    let name = name(argument);
    let expected = if argument.get_action().takes_values() {
        description(argument)
    } else {
        "no value".to_owned()
    };
    let supplied = supplied_value(words, &name, 0);
    let value = text(error, ContextKind::InvalidValue)
        .filter(|value| !value.is_empty() || supplied.is_some())
        .or(supplied);
    invalid(
        match value {
            Some(value) => format!("{name} expects {expected}; got {value:?}"),
            None => format!("{name} expects {expected}"),
        },
        path,
        Some(&name),
        value,
        &expected,
        None,
    )
}

fn supplied_value<'a>(words: &'a [String], option: &str, occurrence: usize) -> Option<&'a str> {
    words
        .iter()
        .enumerate()
        .take_while(|(_, word)| *word != "--")
        .filter_map(|(index, word)| {
            if word == option {
                Some(words.get(index + 1).filter(|word| !word.starts_with('-')).map(String::as_str))
            } else {
                word.strip_prefix(option).and_then(|suffix| suffix.strip_prefix('=')).map(Some)
            }
        })
        .nth(occurrence)
        .flatten()
}

fn name(argument: &Arg) -> String {
    argument.get_long().map_or_else(|| argument.get_id().to_string(), |long| format!("--{long}"))
}

fn description(argument: &Arg) -> String {
    let description = argument.get_help().map(ToString::to_string).unwrap_or_default();
    let mut letters = description.chars();
    match letters.next() {
        Some(first) => first.to_lowercase().chain(letters).collect(),
        None => description,
    }
}
