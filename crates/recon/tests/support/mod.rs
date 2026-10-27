use std::sync::LazyLock;

use recon::cli::registry::registry;
use recon::cli::schema::output_schema;
use recon::error::ErrorReport;
use serde::Serialize;

pub(crate) fn validate_failure(report: &impl Serialize, command: &[&str]) {
    static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
        jsonschema::draft202012::new(&output_schema::<ErrorReport<'static>>()).unwrap()
    });
    let validator = &*VALIDATOR;
    let envelope = serde_json::to_value(report).unwrap();
    let errors =
        validator.iter_errors(&envelope).map(|error| error.to_string()).collect::<Vec<_>>();
    assert!(errors.is_empty(), "{errors:?}: {envelope}");
    let resolved = envelope["error"]["command"]
        .as_array()
        .map(|path| path.iter().map(|word| word.as_str().unwrap()).collect::<Vec<_>>())
        .unwrap_or_default();
    let command = if command.is_empty() { resolved.as_slice() } else { command };
    if !command.is_empty() {
        let registry = registry();
        let entry = registry.iter().find(|entry| entry.path == command).unwrap();
        assert!(
            entry
                .errors
                .iter()
                .any(|code| serde_json::to_value(code).unwrap() == envelope["error"]["code"]),
            "{}: undeclared code {}",
            command.join(" "),
            envelope["error"]["code"]
        );
    }
}
