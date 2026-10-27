use crate::error::Failure;
use crate::ui::request::UiRequest;

/// One `recon` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    App(AppRequest),
    Ui(UiRequest),
    Help(String),
    Version(String),
    Schema(Vec<String>),
    Context,
}

/// One `recon app` command. A target is an absolute bundle path, a name, or a bundle identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppRequest {
    List,
    Inspect { target: String },
    Launch { target: String },
}

pub fn parse_arguments(arguments: &[String]) -> Result<Request, Failure> {
    crate::cli::parse(arguments)
}
