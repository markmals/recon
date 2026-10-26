use super::decimal;
use super::registry::{CommandSpec, parser};

pub(super) fn render(registry: &[CommandSpec], path: &[&str]) -> String {
    if path.is_empty() {
        return top(registry);
    }
    let mut parser = parser(registry);
    parser.build();
    let mut command = &mut parser;
    for word in path {
        let Some(child) = command.find_subcommand_mut(word) else {
            return top(registry);
        };
        command = child;
    }
    let mut output = command
        .clone()
        .bin_name(format!("recon {}", path.join(" ")))
        .render_long_help()
        .to_string();
    output.push_str("\nOrientation: --help, -h; recon help <command>.\n");
    if let Some(entry) = registry.iter().find(|entry| entry.path == path) {
        output.insert_str(0, &format!("{}\n\n", entry.summary));
        output.push_str(&format!(
            "\nEffects: {}\nPermissions: {}\nHosts: {}\nErrors: {}\n",
            list(entry.effects),
            entry
                .permissions
                .iter()
                .map(|(permission, condition)| condition.map_or_else(
                    || (*permission).to_owned(),
                    |condition| format!("{permission} ({condition})")
                ))
                .collect::<Vec<_>>()
                .join(", "),
            list(entry.hosts),
            entry
                .errors
                .iter()
                .map(|code| serde_json::to_value(code)
                    .expect("error code")
                    .as_str()
                    .expect("string code")
                    .to_owned())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        if entry.permissions.is_empty() {
            output.push_str("No privacy permission is required.\n");
        }
        for argument in command.get_arguments() {
            if let Some(constraints) = decimal::constraints(argument.get_value_parser()) {
                output.push_str(&format!(
                    "--{}: ASCII decimal digits only; minimum {}; maximum {}.\n",
                    argument.get_long().expect("numeric option"),
                    constraints.minimum,
                    constraints.maximum
                ));
            }
        }
    }
    output
}

fn list(words: &[&str]) -> String {
    if words.is_empty() { "none".into() } else { words.join(", ") }
}

fn top(registry: &[CommandSpec]) -> String {
    let mut output = "Recon: inspect applications without activating them.\n\nUsage: recon <command> [arguments] [options]\n\nCommands:\n".to_owned();
    for group in ["app", "ui", "meta-commands"] {
        output.push_str(&format!("\n{group}:\n"));
        for entry in registry.iter().filter(|entry| {
            if group == "meta-commands" { entry.path.len() == 1 } else { entry.path[0] == group }
        }) {
            output.push_str(&format!("  {:20} {}\n", entry.path.join(" "), entry.summary));
        }
    }
    output.push_str("\nOrientation:\n  --help, -h  Show help for a group or command\n  --version   Show Recon's version and build commit\n  help        Show help for a command path\n\nEvery command prints one JSON object to standard output on success and one JSON error object to standard error on failure. recon schema lists every command and the error contract; recon schema <command> gives one command's full specification. recon context reports this installation's permissions and capabilities.\n");
    output
}
