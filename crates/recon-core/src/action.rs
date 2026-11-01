//! What an extension exports, and how the host runs one of its actions.
//!
//! An action's identity is the extension that owns it plus the name its author's function
//! carries; renaming that function changes the export. The host routes an invocation by that
//! identity and exchanges serialized payloads with the extension, so nothing here names a
//! capability, a transport, or an operation an extension happens to implement.

use std::fmt;

use serde_json::{Map, Value};

use crate::error::{ErrorCode, Failure, InspectionFailure, InspectionStage};

/// The identifier of one extension: lowercase ASCII letters, digits, and hyphens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtensionId(&'static str);

impl ExtensionId {
    /// # Panics
    ///
    /// When `id` is not a valid extension identifier. Declare an identity as a `const` item and
    /// an invalid spelling fails to compile rather than reaching a client.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        assert!(
            is_extension_id(id),
            "an extension identifier is lowercase letters, digits and hyphens"
        );
        Self(id)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ExtensionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// The exported name of one action: the base identifier of its author's function, lowerCamelCase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionName(&'static str);

impl ActionName {
    /// # Panics
    ///
    /// When `name` is not a valid export name, which a `const` declaration catches at compile
    /// time.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        assert!(is_action_name(name), "an export name is lowerCamelCase ASCII letters and digits");
        Self(name)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for ActionName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// One action's identity: the extension that exports it, and the name it is exported under.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionId {
    extension: ExtensionId,
    name: ActionName,
}

impl ActionId {
    #[must_use]
    pub const fn new(extension: ExtensionId, name: &'static str) -> Self {
        Self { extension, name: ActionName::new(name) }
    }

    #[must_use]
    pub const fn extension(self) -> ExtensionId {
        self.extension
    }

    #[must_use]
    pub const fn name(self) -> ActionName {
        self.name
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}", self.extension, self.name)
    }
}

/// One request to run an exported action, carrying the serialized input its author declared.
#[derive(Debug)]
pub struct Invocation {
    pub action: ActionId,
    pub input: Map<String, Value>,
}

/// An extension the host can run exported actions in.
///
/// The implementor owns the target or session an invocation applies to; the identity of a
/// backend-local handle means nothing outside it, so the host never supplies one.
pub trait Extension {
    /// Every action this extension exports, in the order it declares them.
    fn exports(&self) -> &'static [ActionId];

    /// Runs one of this extension's own exports and returns its serialized result.
    fn run(&self, invocation: Invocation) -> Result<Map<String, Value>, Failure>;
}

/// Runs `invocation` in the extension that exports its action.
///
/// An action the extension does not export is a failure rather than a silent substitution: no
/// other implementation stands in for a missing one.
///
/// # Errors
///
/// [`ErrorCode::InspectionFailed`] when the extension exports no action of that identity.
pub fn invoke(
    extension: &impl Extension,
    invocation: Invocation,
) -> Result<Map<String, Value>, Failure> {
    if !extension.exports().contains(&invocation.action) {
        return Err(Failure::new(
            ErrorCode::InspectionFailed,
            format!("no implementation of the action {}", invocation.action),
            InspectionFailure::new(InspectionStage::Read)
                .detail(format!(
                    "the extension exports no action named {}",
                    invocation.action.name()
                ))
                .details(),
        ));
    }
    extension.run(invocation)
}

const fn is_extension_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        let allowed = byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-';
        if !allowed {
            return false;
        }
        index += 1;
    }
    true
}

const fn is_action_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut index = 1;
    while index < bytes.len() {
        if !bytes[index].is_ascii_alphanumeric() {
            return false;
        }
        index += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO: ExtensionId = ExtensionId::new("demo-extension");

    struct Demo;

    impl Extension for Demo {
        fn exports(&self) -> &'static [ActionId] {
            const EXPORTS: [ActionId; 1] = [ActionId::new(DEMO, "greeting")];
            &EXPORTS
        }

        fn run(&self, invocation: Invocation) -> Result<Map<String, Value>, Failure> {
            let mut result = Map::new();
            result.insert("action".to_owned(), Value::from(invocation.action.to_string()));
            Ok(result)
        }
    }

    #[test]
    fn an_action_is_named_by_its_extension_and_its_function() {
        let action = ActionId::new(DEMO, "greeting");
        assert_eq!(action.to_string(), "demo-extension.greeting");
        assert_eq!(action.extension(), DEMO);
        assert_eq!(action.name().as_str(), "greeting");
    }

    #[test]
    fn an_exported_action_runs_in_its_own_extension() {
        let invocation = Invocation { action: ActionId::new(DEMO, "greeting"), input: Map::new() };
        let result = invoke(&Demo, invocation).expect("the export runs");
        assert_eq!(result["action"], Value::from("demo-extension.greeting"));
    }

    #[test]
    fn an_undeclared_action_is_an_inspection_failure_rather_than_a_substitution() {
        let invocation = Invocation { action: ActionId::new(DEMO, "windows"), input: Map::new() };
        let failure = invoke(&Demo, invocation).expect_err("no implementation");
        assert_eq!(failure.code, ErrorCode::InspectionFailed);
        assert!(failure.message.contains("demo-extension.windows"), "{}", failure.message);
    }

    #[test]
    fn identities_reject_spellings_an_author_cannot_export() {
        assert!(!is_extension_id(""));
        assert!(!is_extension_id("AppKit"));
        assert!(is_extension_id("appkit"));
        assert!(!is_action_name("Windows"));
        assert!(!is_action_name("read_tree"));
        assert!(is_action_name("readTree2"));
    }
}
