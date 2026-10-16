//! Turning the Core Foundation values macOS returns for one element into the attributes a tree
//! node carries.

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_application_services::{AXError, AXUIElement, AXValue, AXValueType};
use objc2_core_foundation::{
    CFArray, CFBoolean, CFNumber, CFRetained, CFString, CFType, CGPoint, CGSize, Type,
};

use super::accessibility::Element;
use crate::ui::tree::AttributeValue;

/// The attributes Recon reads, in the order it requests them.
#[derive(Clone, Copy)]
pub enum Attribute {
    Role,
    Subrole,
    Title,
    Description,
    Value,
    Identifier,
    Enabled,
    Focused,
    Position,
    Size,
    Children,
}

impl Attribute {
    pub const ALL: [Attribute; 11] = [
        Attribute::Role,
        Attribute::Subrole,
        Attribute::Title,
        Attribute::Description,
        Attribute::Value,
        Attribute::Identifier,
        Attribute::Enabled,
        Attribute::Focused,
        Attribute::Position,
        Attribute::Size,
        Attribute::Children,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Attribute::Role => "AXRole",
            Attribute::Subrole => "AXSubrole",
            Attribute::Title => "AXTitle",
            Attribute::Description => "AXDescription",
            Attribute::Value => "AXValue",
            Attribute::Identifier => "AXIdentifier",
            Attribute::Enabled => "AXEnabled",
            Attribute::Focused => "AXFocused",
            Attribute::Position => "AXPosition",
            Attribute::Size => "AXSize",
            Attribute::Children => "AXChildren",
        }
    }
}

/// The error macOS returned instead of a value, or `None` when there is a value. A bulk read
/// reports each attribute it could not supply as an `AXValue` holding the error for that one
/// attribute, which is not always the same as the element having no such attribute.
pub fn attribute_error(value: &CFType) -> Option<AXError> {
    let value = value.downcast_ref::<AXValue>()?;
    // SAFETY: reading the kind of an AXValue does not message the application.
    if unsafe { value.r#type() } != AXValueType::AXError {
        return None;
    }
    let mut error = AXError::Failure;
    // SAFETY: an AXValue of kind AXError wraps an AXError, which `error` is.
    unsafe { value.value(AXValueType::AXError, NonNull::from(&mut error).cast()) };
    Some(error)
}

pub fn text(value: &CFType) -> Option<String> {
    Some(value.downcast_ref::<CFString>()?.to_string())
}

pub fn boolean(value: &CFType) -> Option<bool> {
    Some(value.downcast_ref::<CFBoolean>()?.as_bool())
}

/// A value Recon can report; anything else is reported as no value at all.
pub fn attribute_value(value: &CFType) -> Option<AttributeValue> {
    if let Some(text) = value.downcast_ref::<CFString>() {
        return Some(AttributeValue::Text(text.to_string()));
    }
    if let Some(flag) = value.downcast_ref::<CFBoolean>() {
        return Some(AttributeValue::Boolean(flag.as_bool()));
    }
    let number = value.downcast_ref::<CFNumber>()?;
    if number.is_float_type() {
        return Some(AttributeValue::Real(number.as_f64()?));
    }
    Some(AttributeValue::Integer(number.as_i64()?))
}

pub fn point(value: &CFType) -> Option<CGPoint> {
    let mut point = CGPoint::ZERO;
    geometry(value, AXValueType::CGPoint, NonNull::from(&mut point).cast()).then_some(point)
}

pub fn size(value: &CFType) -> Option<CGSize> {
    let mut size = CGSize::ZERO;
    geometry(value, AXValueType::CGSize, NonNull::from(&mut size).cast()).then_some(size)
}

/// The children of an element: the accessibility elements it lists, and whether it listed
/// anything else. An application controls what goes in `AXChildren`, so a member that is not an
/// element is a child Recon could not read rather than one it quietly drops.
#[derive(Default)]
pub struct Children {
    pub elements: Vec<Element>,
    pub unreadable: bool,
}

pub fn children_of(children: &CFType) -> Option<Children> {
    let children = children.downcast_ref::<CFArray>()?.retain();
    // SAFETY: the members of a Core Foundation array are Core Foundation values.
    let children: CFRetained<CFArray<CFType>> = unsafe { CFRetained::cast_unchecked(children) };
    let mut read = Children::default();
    for child in children.iter() {
        match child.downcast::<AXUIElement>() {
            Ok(element) => read.elements.push(Element::new(element)),
            Err(_) => read.unreadable = true,
        }
    }
    Some(read)
}

fn geometry(value: &CFType, kind: AXValueType, into: NonNull<c_void>) -> bool {
    let Some(value) = value.downcast_ref::<AXValue>() else {
        return false;
    };
    // SAFETY: `into` points at the geometry type `kind` names, and the kind is checked first.
    unsafe { value.r#type() == kind && value.value(kind, into) }
}

#[cfg(test)]
mod tests {
    use objc2_core_foundation::CFRetained;

    use super::*;

    fn error_value(error: AXError) -> CFRetained<AXValue> {
        // SAFETY: an AXValue of kind AXError wraps an AXError, which is what a bulk read returns
        // for an attribute it could not supply.
        unsafe { AXValue::new(AXValueType::AXError, NonNull::from(&error).cast()) }
            .expect("an AXValue")
    }

    fn unsupported_attribute() -> CFRetained<AXValue> {
        error_value(AXError::AttributeUnsupported)
    }

    #[test]
    fn the_attributes_are_requested_in_the_order_they_are_read_back() {
        for (index, attribute) in Attribute::ALL.iter().enumerate() {
            assert_eq!(index, *attribute as usize, "{} is out of order", attribute.name());
        }
    }

    #[test]
    fn an_attribute_that_has_no_value_reports_the_error_macos_gave_instead() {
        assert_eq!(attribute_error(&unsupported_attribute()), Some(AXError::AttributeUnsupported));
        assert_eq!(
            attribute_error(&error_value(AXError::CannotComplete)),
            Some(AXError::CannotComplete)
        );
        assert_eq!(attribute_error(&CFString::from_str("AXButton")), None);
        assert_eq!(attribute_error(&CFNumber::new_i64(7)), None);
    }

    #[test]
    fn a_child_that_is_not_an_accessibility_element_is_a_child_recon_could_not_read() {
        let text = CFString::from_str("not an element");
        let number = CFNumber::new_i64(7);
        let mixed = CFArray::from_objects(&[&*text as &CFType, &number]);

        let children = children_of(&mixed).expect("a child array");
        assert!(children.elements.is_empty());
        assert!(children.unreadable);

        let empty = CFArray::<CFType>::from_objects(&[]);
        let children = children_of(&empty).expect("a child array");
        assert!(children.elements.is_empty());
        assert!(!children.unreadable);

        assert!(children_of(&text).is_none());
    }

    #[test]
    fn a_value_is_reported_only_when_it_is_text_a_number_or_a_flag() {
        assert_eq!(
            attribute_value(&CFString::from_str("fixture value")),
            Some(AttributeValue::Text("fixture value".to_owned()))
        );
        assert_eq!(attribute_value(CFBoolean::new(true)), Some(AttributeValue::Boolean(true)));
        assert_eq!(attribute_value(&CFNumber::new_i64(42)), Some(AttributeValue::Integer(42)));
        assert_eq!(attribute_value(&CFNumber::new_f64(0.5)), Some(AttributeValue::Real(0.5)));
        assert_eq!(attribute_value(&unsupported_attribute()), None);
    }

    #[test]
    fn a_frame_needs_both_a_position_and_a_size() {
        let origin = CGPoint::new(12.0, 34.0);
        // SAFETY: an AXValue of kind CGPoint wraps a CGPoint.
        let position = unsafe { AXValue::new(AXValueType::CGPoint, NonNull::from(&origin).cast()) }
            .expect("an AXValue");
        let extent = CGSize::new(800.0, 600.0);
        // SAFETY: an AXValue of kind CGSize wraps a CGSize.
        let extent_value =
            unsafe { AXValue::new(AXValueType::CGSize, NonNull::from(&extent).cast()) }
                .expect("an AXValue");

        assert_eq!(point(&position), Some(origin));
        assert_eq!(size(&extent_value), Some(extent));
        assert_eq!(point(&extent_value), None);
        assert_eq!(size(&position), None);
    }
}
