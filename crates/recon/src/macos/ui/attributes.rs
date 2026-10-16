//! Turning the Core Foundation values macOS returns for one element into the attributes a tree
//! node carries.

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_application_services::{AXUIElement, AXValue, AXValueType};
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

/// macOS reports an attribute an element does not have as an `AXValue` holding an error.
pub fn is_missing(value: &CFType) -> bool {
    value
        .downcast_ref::<AXValue>()
        // SAFETY: reading the kind of an AXValue does not message the application.
        .is_some_and(|value| unsafe { value.r#type() } == AXValueType::AXError)
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

pub fn elements(children: &CFType) -> Option<Vec<Element>> {
    let children = children.downcast_ref::<CFArray>()?.retain();
    // SAFETY: the children of an accessibility element are accessibility elements.
    let children: CFRetained<CFArray<AXUIElement>> =
        unsafe { CFRetained::cast_unchecked(children) };
    Some(children.iter().map(Element::new).collect())
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
    use objc2_application_services::AXError;
    use objc2_core_foundation::CFRetained;

    use super::*;

    fn unsupported_attribute() -> CFRetained<AXValue> {
        let error = AXError::AttributeUnsupported;
        // SAFETY: an AXValue of kind AXError wraps an AXError, which is what macOS returns for an
        // attribute an element does not have.
        unsafe { AXValue::new(AXValueType::AXError, NonNull::from(&error).cast()) }
            .expect("an AXValue")
    }

    #[test]
    fn the_attributes_are_requested_in_the_order_they_are_read_back() {
        for (index, attribute) in Attribute::ALL.iter().enumerate() {
            assert_eq!(index, *attribute as usize, "{} is out of order", attribute.name());
        }
    }

    #[test]
    fn an_attribute_an_element_does_not_have_reads_as_missing() {
        assert!(is_missing(&unsupported_attribute()));
        assert!(!is_missing(&CFString::from_str("AXButton")));
        assert!(!is_missing(&CFNumber::new_i64(7)));
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
