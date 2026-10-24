//! The `recon` command line: its two command groups, the `app` commands, and the usage text that
//! every argument mistake reports.

use crate::ui::request::{UiRequest, parse_ui_arguments};

pub const USAGE: &str = "usage: recon app list | inspect <target> | launch <target>
       recon ui windows <target> [--pid <pid>]
       recon ui tree <target> [--pid <pid>] [--window <id>]
       recon ui screenshot <target> [--pid <pid>] [--window <id>] --out <file>";

/// One `recon` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    App(AppRequest),
    Ui(UiRequest),
}

/// One `recon app` command. A target is an absolute bundle path, a name, or a bundle identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppRequest {
    List,
    Inspect { target: String },
    Launch { target: String },
}

/// Parses the arguments that follow `recon`.
pub fn parse_arguments(arguments: &[String]) -> Result<Request, String> {
    match arguments.split_first() {
        Some((group, rest)) if group == "app" => parse_app_arguments(rest).map(Request::App),
        Some((group, rest)) if group == "ui" => parse_ui_arguments(rest).map(Request::Ui),
        _ => Err(USAGE.to_owned()),
    }
}

fn parse_app_arguments(arguments: &[String]) -> Result<AppRequest, String> {
    match arguments {
        [command] if command == "list" => Ok(AppRequest::List),
        [command, target] if command == "inspect" => {
            Ok(AppRequest::Inspect { target: target.clone() })
        }
        [command, target] if command == "launch" => {
            Ok(AppRequest::Launch { target: target.clone() })
        }
        _ => Err(USAGE.to_owned()),
    }
}
