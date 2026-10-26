use std::sync::LazyLock;

use recon::cli::registry::registry;
use recon::cli::schema::output_schema;
use recon::error::{ErrorReport, Failure};

pub(crate) fn validate_failure(failure: &Failure, command: &[&str]) {
    static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
        jsonschema::draft202012::new(&output_schema::<ErrorReport<'static>>()).unwrap()
    });
    let validator = &*VALIDATOR;
    let failure = failure.clone().with_command(command);
    let envelope = serde_json::to_value(failure.report()).unwrap();
    let errors =
        validator.iter_errors(&envelope).map(|error| error.to_string()).collect::<Vec<_>>();
    assert!(errors.is_empty(), "{errors:?}: {envelope}");
    let registry = registry();
    let entry = registry.iter().find(|entry| entry.path == command).unwrap();
    assert!(
        entry.errors.contains(&failure.code),
        "{}: {:?} undeclared",
        command.join(" "),
        failure.code
    );
}
