use std::sync::LazyLock;

use super::{ErrorReport, Failure};
use crate::schema::output_schema;

/// Every failure a test builds is held to the envelope schema the executable publishes, so a
/// detail object that no longer matches its contract fails where it is written rather than in
/// whichever command happens to print it.
pub(super) fn validate(failure: &Failure) {
    static VALIDATOR: LazyLock<jsonschema::Validator> = LazyLock::new(|| {
        jsonschema::draft202012::new(&output_schema::<ErrorReport<'static>>()).unwrap()
    });
    let envelope = serde_json::to_value(failure.report()).unwrap();
    let errors =
        VALIDATOR.iter_errors(&envelope).map(|error| error.to_string()).collect::<Vec<_>>();
    assert!(errors.is_empty(), "{errors:?}: {envelope}");
}
