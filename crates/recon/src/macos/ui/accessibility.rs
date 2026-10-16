//! Reading one accessibility element out of a running application. Recon only reads: it never
//! sets an attribute, performs an action, or opens a menu.

use std::ffi::c_float;
use std::hash::{Hash, Hasher};
use std::ptr::{self, NonNull};

use libc::pid_t;
use objc2_application_services::{AXCopyMultipleAttributeOptions, AXError, AXUIElement};
use objc2_core_foundation::{CFArray, CFRetained, CFString, CFType};

use super::attributes::{self, Attribute};
use crate::ui::tree::{ElementAttributes, ElementSource, ReadFailure};
use crate::ui::window::Bounds;

/// Bounds every accessibility request, so an unresponsive application cannot hang Recon.
const MESSAGING_TIMEOUT: c_float = 2.0;

/// An accessibility element. Elements have no durable identity, but macOS can still tell whether
/// two of them refer to the same object, which is what a tree read needs to spot repetition.
#[derive(Clone, Debug)]
pub struct Element(CFRetained<AXUIElement>);

impl Element {
    pub fn new(element: CFRetained<AXUIElement>) -> Self {
        Self(element)
    }
}

impl PartialEq for Element {
    fn eq(&self, other: &Self) -> bool {
        *self.0 == *other.0
    }
}

impl Eq for Element {}

impl Hash for Element {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (*self.0).hash(state);
    }
}

/// Reads elements of a running application, one request per element.
pub struct Accessibility {
    attributes: CFRetained<CFArray<CFString>>,
}

impl Accessibility {
    pub fn new() -> Self {
        let names: Vec<CFRetained<CFString>> =
            Attribute::ALL.iter().map(|attribute| CFString::from_str(attribute.name())).collect();
        Self { attributes: CFArray::from_retained_objects(&names) }
    }

    fn values_of(&self, element: &Element) -> Result<Vec<CFRetained<CFType>>, ReadFailure> {
        let mut values: *const CFArray = ptr::null();
        // SAFETY: one request for every attribute; on success `values` owns the returned array.
        let error = unsafe {
            element.0.copy_multiple_attribute_values(
                self.attributes.as_opaque(),
                AXCopyMultipleAttributeOptions::empty(),
                NonNull::from(&mut values),
            )
        };
        if error != AXError::Success {
            return Err(read_failure(error));
        }
        let values =
            NonNull::new(values.cast_mut()).ok_or(ReadFailure::Failed(AXError::Failure.0))?;
        // SAFETY: the array is owned, and holds one Core Foundation value per attribute.
        let values: CFRetained<CFArray<CFType>> =
            unsafe { CFRetained::cast_unchecked(CFRetained::from_raw(values)) };
        Ok(values.to_vec())
    }
}

impl ElementSource for Accessibility {
    type Element = Element;

    fn read(&mut self, element: &Element) -> Result<ElementAttributes<Element>, ReadFailure> {
        let values = self.values_of(element)?;
        if let Some(failure) = values
            .iter()
            .filter_map(|value| attributes::attribute_error(value))
            .find_map(attribute_failure)
        {
            return Err(failure);
        }
        let value_of = |attribute: Attribute| {
            values
                .get(attribute as usize)
                .filter(|value| attributes::attribute_error(value).is_none())
                .cloned()
        };
        let position = value_of(Attribute::Position).and_then(|value| attributes::point(&value));
        let size = value_of(Attribute::Size).and_then(|value| attributes::size(&value));

        Ok(ElementAttributes {
            role: value_of(Attribute::Role).and_then(|value| attributes::text(&value)),
            subrole: value_of(Attribute::Subrole).and_then(|value| attributes::text(&value)),
            title: value_of(Attribute::Title).and_then(|value| attributes::text(&value)),
            description: value_of(Attribute::Description)
                .and_then(|value| attributes::text(&value)),
            value: value_of(Attribute::Value).and_then(|value| attributes::attribute_value(&value)),
            identifier: value_of(Attribute::Identifier).and_then(|value| attributes::text(&value)),
            enabled: value_of(Attribute::Enabled).and_then(|value| attributes::boolean(&value)),
            focused: value_of(Attribute::Focused).and_then(|value| attributes::boolean(&value)),
            frame: position.zip(size).map(|(position, size)| Bounds {
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
            }),
            actions: actions_of(element)?,
            children: value_of(Attribute::Children)
                .and_then(|children| attributes::elements(&children))
                .unwrap_or_default(),
        })
    }
}

/// Bounds how long every accessibility request this process makes may take, so that an
/// unresponsive application cannot hang Recon.
pub fn bound_messaging() {
    // SAFETY: the system-wide element carries process-wide accessibility settings like this one.
    unsafe {
        let system_wide = AXUIElement::new_system_wide();
        system_wide.set_messaging_timeout(MESSAGING_TIMEOUT);
    }
}

/// The top-level element of a running application.
pub fn application_element(pid: u32) -> Result<CFRetained<AXUIElement>, String> {
    let pid = pid_t::try_from(pid).map_err(|_| format!("{pid} is not a process id"))?;
    // SAFETY: creating an application element does not message the application.
    Ok(unsafe { AXUIElement::new_application(pid) })
}

/// The value of one attribute. The error tells the caller why there is none: `kAXErrorNoValue`
/// and `kAXErrorAttributeUnsupported` mean the element simply does not have the attribute, while
/// `kAXErrorCannotComplete` means the application did not answer.
pub fn attribute(element: &AXUIElement, name: &str) -> Result<CFRetained<CFType>, AXError> {
    let mut value: *const CFType = ptr::null();
    // SAFETY: on success `value` owns the attribute's value.
    let error = unsafe {
        element.copy_attribute_value(&CFString::from_str(name), NonNull::from(&mut value))
    };
    if error != AXError::Success {
        return Err(error);
    }
    NonNull::new(value.cast_mut())
        // SAFETY: the value is owned by this process.
        .map(|value| unsafe { CFRetained::from_raw(value) })
        .ok_or(AXError::NoValue)
}

/// How a failed request to one element is reported: the tree keeps going around it.
fn read_failure(error: AXError) -> ReadFailure {
    match error {
        AXError::InvalidUIElement => ReadFailure::Unavailable,
        AXError::CannotComplete => ReadFailure::Timeout,
        other => ReadFailure::Failed(other.0),
    }
}

/// Whether the error a bulk read gave instead of one attribute's value is a failure of the whole
/// element. Only an attribute the element does not have is ordinary; any other error means the
/// application could not supply the value, which a node reporting `null` or `[]` would hide.
fn attribute_failure(error: AXError) -> Option<ReadFailure> {
    match error {
        AXError::NoValue | AXError::AttributeUnsupported => None,
        other => Some(read_failure(other)),
    }
}

/// Whether an action-name request that did not return names is a failure of the whole element.
/// An element that advertises no action is ordinary; an element that stopped answering is not.
fn action_failure(error: AXError) -> Option<ReadFailure> {
    match error {
        AXError::Success
        | AXError::NoValue
        | AXError::AttributeUnsupported
        | AXError::ActionUnsupported => None,
        other => Some(read_failure(other)),
    }
}

fn actions_of(element: &Element) -> Result<Vec<String>, ReadFailure> {
    let mut names: *const CFArray = ptr::null();
    // SAFETY: on success `names` owns an array of action names.
    let error = unsafe { element.0.copy_action_names(NonNull::from(&mut names)) };
    if let Some(failure) = action_failure(error) {
        return Err(failure);
    }
    let Some(names) = NonNull::new(names.cast_mut()) else {
        return Ok(Vec::new());
    };
    // SAFETY: the array is owned, and action names are strings.
    let names: CFRetained<CFArray<CFString>> =
        unsafe { CFRetained::cast_unchecked(CFRetained::from_raw(names)) };
    Ok(names.iter().map(|name| name.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vanished_element_is_told_apart_from_a_timeout_and_from_other_failures() {
        assert_eq!(read_failure(AXError::InvalidUIElement), ReadFailure::Unavailable);
        assert_eq!(read_failure(AXError::CannotComplete), ReadFailure::Timeout);
        assert_eq!(read_failure(AXError::NotImplemented), ReadFailure::Failed(-25208));
        assert_eq!(read_failure(AXError::APIDisabled), ReadFailure::Failed(-25211));
    }

    #[test]
    fn only_a_truly_absent_attribute_leaves_the_element_readable() {
        assert_eq!(attribute_failure(AXError::NoValue), None);
        assert_eq!(attribute_failure(AXError::AttributeUnsupported), None);
    }

    #[test]
    fn an_attribute_the_application_could_not_supply_fails_the_element() {
        assert_eq!(attribute_failure(AXError::CannotComplete), Some(ReadFailure::Timeout));
        assert_eq!(attribute_failure(AXError::InvalidUIElement), Some(ReadFailure::Unavailable));
        assert_eq!(attribute_failure(AXError::Failure), Some(ReadFailure::Failed(-25200)));
        assert_eq!(attribute_failure(AXError::NotImplemented), Some(ReadFailure::Failed(-25208)));
    }

    #[test]
    fn an_element_that_advertises_no_action_is_not_a_failed_read() {
        assert_eq!(action_failure(AXError::Success), None);
        assert_eq!(action_failure(AXError::NoValue), None);
        assert_eq!(action_failure(AXError::AttributeUnsupported), None);
        assert_eq!(action_failure(AXError::ActionUnsupported), None);
    }

    #[test]
    fn an_action_request_that_fails_fails_the_whole_element() {
        assert_eq!(action_failure(AXError::CannotComplete), Some(ReadFailure::Timeout));
        assert_eq!(action_failure(AXError::InvalidUIElement), Some(ReadFailure::Unavailable));
        assert_eq!(action_failure(AXError::Failure), Some(ReadFailure::Failed(-25200)));
    }
}
