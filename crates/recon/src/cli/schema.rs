use clap::ArgAction;
use schemars::{JsonSchema, generate::SchemaSettings};
use serde::Serialize;
use serde_json::{Value, json};

use super::decimal;
use super::output_path;
use super::registry::{CommandSpec, registry};
use crate::error::{ErrorCode, ErrorReport, Failure};

pub fn output_schema<T: JsonSchema>() -> Value {
    serde_json::to_value(
        SchemaSettings::draft2020_12().for_serialize().into_generator().into_root_schema_for::<T>(),
    )
    .expect("JSON Schema serializes")
}

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
pub enum SchemaResponse {
    Index(Index),
    Command(Specification),
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Index {
    schema_version: u32,
    recon: &'static str,
    commands: Vec<CommandSummary>,
    errors: Errors,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CommandSummary {
    path: Vec<String>,
    summary: String,
    effects: Vec<String>,
    hosts: Vec<String>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Errors {
    schema: Value,
    codes: Vec<Code>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Code {
    code: ErrorCode,
    exit_code: i32,
    retryable: bool,
    summary: &'static str,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Specification {
    schema_version: u32,
    recon: &'static str,
    command: Description,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Description {
    path: Vec<String>,
    summary: String,
    description: String,
    arguments: Vec<Argument>,
    options: Vec<OptionDescription>,
    effects: Vec<String>,
    permissions: Vec<Permission>,
    hosts: Vec<String>,
    output: Value,
    errors: Vec<ErrorCode>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Argument {
    name: String,
    value_name: String,
    #[serde(rename = "type")]
    kind: String,
    required: bool,
    description: String,
    constraints: Option<Value>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OptionDescription {
    name: String,
    value_name: String,
    #[serde(rename = "type")]
    kind: String,
    required: bool,
    default: Option<Value>,
    repeatable: bool,
    description: String,
    constraints: Option<Value>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Permission {
    permission: String,
    /// Human-readable condition only; it is not machine-evaluable.
    condition: Option<String>,
}

pub fn describe(path: &[String]) -> Result<SchemaResponse, Failure> {
    let registry = registry();
    if path.is_empty() {
        return Ok(SchemaResponse::Index(Index {
            schema_version: 1,
            recon: env!("CARGO_PKG_VERSION"),
            commands: registry
                .iter()
                .map(|entry| CommandSummary {
                    path: strings(entry.path),
                    summary: entry.summary.into(),
                    effects: strings(entry.effects),
                    hosts: strings(entry.hosts),
                })
                .collect(),
            errors: Errors {
                schema: output_schema::<ErrorReport<'static>>(),
                codes: ErrorCode::ALL
                    .into_iter()
                    .map(|code| Code {
                        code,
                        exit_code: code.exit_code(),
                        retryable: code.retryable(),
                        summary: code.summary(),
                    })
                    .collect(),
            },
        }));
    }
    let entry = registry.iter().find(|entry| entry.path.iter().copied().eq(path.iter().map(String::as_str))).ok_or_else(|| Failure::new(ErrorCode::InvalidArgument, format!("no command named {}", path.join(" ")), json!({"argument":null,"value":path.join(" "),"expected":"a registered command path","accepted":registry.iter().map(|entry| entry.path.join(" ")).collect::<Vec<_>>()})).with_command(&["schema"]))?;
    Ok(SchemaResponse::Command(Specification {
        schema_version: 1,
        recon: env!("CARGO_PKG_VERSION"),
        command: description(entry),
    }))
}

fn description(entry: &CommandSpec) -> Description {
    let mut parser = entry.parser();
    parser.build();
    let mut arguments = Vec::new();
    let mut options = Vec::new();
    for argument in parser.get_arguments() {
        let value_name = argument
            .get_value_names()
            .and_then(|names| names.first())
            .map(ToString::to_string)
            .unwrap_or_default();
        let numeric = decimal::constraints(argument.get_value_parser());
        let kind = if numeric.is_some() { "integer" } else { "string" }.to_owned();
        let constraints = numeric
            .map(|constraints| {
                serde_json::to_value(constraints).expect("numeric constraints serialize")
            })
            .or_else(|| output_path::constraints(argument.get_value_parser()));
        let description = argument.get_help().map(ToString::to_string).unwrap_or_default();
        if argument.is_positional() {
            arguments.push(Argument {
                name: argument.get_id().to_string(),
                value_name,
                kind,
                required: argument.is_required_set(),
                description,
                constraints,
            });
        } else {
            let default = argument
                .get_default_values()
                .first()
                .map(|value| Value::String(value.to_string_lossy().into_owned()));
            options.push(OptionDescription {
                name: format!("--{}", argument.get_long().expect("long option")),
                value_name,
                kind,
                required: argument.is_required_set(),
                default,
                repeatable: matches!(argument.get_action(), ArgAction::Append | ArgAction::Count),
                description,
                constraints,
            });
        }
    }
    Description {
        path: strings(entry.path),
        summary: entry.summary.into(),
        description: entry.description.into(),
        arguments,
        options,
        effects: strings(entry.effects),
        permissions: entry
            .permissions
            .iter()
            .map(|(permission, condition)| Permission {
                permission: (*permission).into(),
                condition: condition.map(str::to_owned),
            })
            .collect(),
        hosts: strings(entry.hosts),
        output: (entry.output)(),
        errors: entry.errors.clone(),
    }
}

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}
