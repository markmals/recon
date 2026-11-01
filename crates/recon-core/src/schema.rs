//! The JSON Schema of a serialized payload.
//!
//! One generator setting describes every payload Recon publishes, so a command's output schema,
//! the error envelope's schema, and a test's validator cannot disagree about what a type encodes.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde_json::Value;

/// The schema of `T` as it serializes, which is the direction every Recon payload travels.
pub fn output_schema<T: JsonSchema>() -> Value {
    serde_json::to_value(
        SchemaSettings::draft2020_12().for_serialize().into_generator().into_root_schema_for::<T>(),
    )
    .expect("JSON Schema serializes")
}
