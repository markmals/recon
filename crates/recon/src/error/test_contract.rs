use std::sync::LazyLock;

use super::{ErrorReport, Failure};
use crate::cli::{registry, schema::output_schema};

pub(super) fn validate(failure: &Failure) {
    static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
        jsonschema::draft202012::new(&output_schema::<ErrorReport<'static>>()).unwrap()
    });
    let envelope = serde_json::to_value(failure.report()).unwrap();
    let errors =
        VALIDATOR.iter_errors(&envelope).map(|error| error.to_string()).collect::<Vec<_>>();
    assert!(errors.is_empty(), "{errors:?}: {envelope}");
    if let Some(command) = &failure.command {
        let registry = registry::registry();
        let entry = registry
            .iter()
            .find(|entry| entry.path.iter().copied().eq(command.iter().map(String::as_str)))
            .unwrap();
        assert!(entry.errors.contains(&failure.code), "{command:?}: undeclared {:?}", failure.code);
    }
}
