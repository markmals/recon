use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::CFString;
use proptest::prelude::*;

use super::accessibility::{Element, menu_bar_value};
use crate::application::Pid;
use crate::error::ErrorCode;

#[test]
fn proposal_0004_only_absent_or_unsupported_menu_attributes_allow_an_unfiltered_tree() {
    let pid = Pid(412);
    for error in [AXError::NoValue, AXError::AttributeUnsupported] {
        assert_eq!(menu_bar_value(Err(error), pid).unwrap(), None);
    }
    assert_eq!(menu_bar_value(Ok(&CFString::from_str("not an element")), pid).unwrap(), None);
}

#[test]
fn proposal_0004_menu_lookup_preserves_element_identity() {
    // SAFETY: creating an application element does not message the process or require it to exist.
    let application = unsafe { AXUIElement::new_application(412) };
    let menu = menu_bar_value(Ok(&application), Pid(412)).unwrap().unwrap();
    assert_eq!(menu, Element::new(application));
}

#[test]
fn proposal_0004_menu_lookup_timeout_is_a_command_failure() {
    let failure = menu_bar_value(Err(AXError::CannotComplete), Pid(412)).unwrap_err();
    assert_eq!(failure.code, ErrorCode::Timeout);
    assert!(failure.retryable);
    let details = serde_json::to_value(&failure.details).unwrap();
    assert_eq!(details["pid"], 412);
    assert_eq!(details["timeoutSeconds"], 2);
}

#[test]
fn proposal_0004_other_menu_lookup_errors_fail_instead_of_returning_an_unfiltered_tree() {
    for error in [
        AXError::Failure,
        AXError::InvalidUIElement,
        AXError::NotImplemented,
        AXError::APIDisabled,
        AXError::IllegalArgument,
        AXError(-25_999),
    ] {
        let failure = menu_bar_value(Err(error), Pid(412)).unwrap_err();
        assert_eq!(failure.code, ErrorCode::Internal);
        assert!(!failure.retryable);
        let details = serde_json::to_value(&failure.details).unwrap();
        let detail = details["detail"].as_str().expect("existing INTERNAL detail string");
        assert!(detail.contains("AXMenuBar"), "{detail}");
        assert!(detail.contains(&error.0.to_string()), "{detail}");
    }
}

proptest! {
    #[test]
    fn proposal_0004_unclassified_menu_lookup_errors_never_return_a_tree(code in any::<i32>()) {
        prop_assume!(![AXError::NoValue.0, AXError::AttributeUnsupported.0, AXError::CannotComplete.0].contains(&code));
        let failure = menu_bar_value(Err(AXError(code)), Pid(412)).unwrap_err();
        prop_assert_eq!(failure.code, ErrorCode::Internal);
        prop_assert!(!failure.retryable);
    }
}
