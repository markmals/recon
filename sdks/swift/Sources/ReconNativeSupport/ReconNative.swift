import AppKit
import CoreGraphics
import Foundation
import QuartzCore

/// The allowlisted way to read a live target object from Swift.
///
/// Every value here comes from one or more messages sent through the Objective-C boundary,
/// which invokes the getter and catches any Objective-C exception before control returns.
/// There is no accessor that takes a selector, a key path, or an expression, and a client can
/// never supply one; adding a field is a change to `Fields`, not something an extension does
/// for itself.
public enum ReconNative {
    public static func string(_ field: ReconStringField, of object: AnyObject)
        -> ReconRead<String>
    {
        let binding = field.binding
        guard applies(binding, to: object) else { return .unavailable }
        guard field.readsAnAppearance else { return Message.text(binding.selector, to: object) }
        switch Message.object(binding.selector, to: object) {
        case .value(let appearance): return Message.text(Fields.name, to: appearance)
        case .unavailable: return .unavailable
        case .failed: return .failed
        }
    }

    /// Whether the object implements the getter at all. An observation reports a getter the
    /// object does not implement as unsupported, which is not the same as having no value.
    public static func responds(to field: ReconStringField, of object: AnyObject) -> Bool {
        let binding = field.binding
        return applies(binding, to: object) && Message.responds(to: binding.selector, object)
    }

    public static func bool(_ field: ReconBoolField, of object: AnyObject) -> ReconRead<Bool> {
        let binding = field.binding
        guard applies(binding, to: object) else { return .unavailable }
        return Message.flag(binding.selector, to: object)
    }

    public static func integer(_ field: ReconIntegerField, of object: AnyObject) -> ReconRead<Int> {
        let binding = field.binding
        guard applies(binding, to: object) else { return .unavailable }
        switch field {
        case .viewSubviewCount, .layerSublayerCount:
            return count(binding.selector, of: object)
        case .screenNumber:
            return screenNumber(of: object)
        default:
            return Message.integer(binding.selector, to: object)
        }
    }

    public static func double(_ field: ReconDoubleField, of object: AnyObject)
        -> ReconRead<Double>
    {
        let binding = field.binding
        guard applies(binding, to: object) else { return .unavailable }
        if let orientation = field.orientation {
            return Message.float(binding.selector, to: object, for: orientation)
        }
        return field.precision == .single
            ? Message.float(binding.selector, to: object)
            : Message.double(binding.selector, to: object)
    }

    public static func rect(_ field: ReconRectField, of object: AnyObject) -> ReconRead<CGRect> {
        let binding = field.binding
        guard applies(binding, to: object) else { return .unavailable }
        return Message.rect(binding.selector, to: object)
    }

    public static func size(_ field: ReconSizeField, of object: AnyObject) -> ReconRead<CGSize> {
        let binding = field.binding
        guard applies(binding, to: object) else { return .unavailable }
        return Message.size(binding.selector, to: object)
    }

    public static func object(_ field: ReconObjectField, of object: AnyObject)
        -> ReconRead<AnyObject>
    {
        let binding = field.binding
        let carriers = field.carriers
        if !carriers.isEmpty {
            guard carriers.contains(where: { Classification.inherits(object, from: $0.objectiveCClass) })
            else { return .unavailable }
        } else {
            guard applies(binding, to: object) else { return .unavailable }
        }
        return Message.object(binding.selector, to: object)
    }

    /// A collection the object carries. A collection it answers nothing for is no children
    /// rather than an unreadable list.
    public static func array(_ field: ReconArrayField, of object: AnyObject)
        -> ReconRead<[AnyObject]>
    {
        let binding = field.binding
        guard applies(binding, to: object) else { return .unavailable }
        switch Message.object(binding.selector, to: object) {
        case .value(let collection): return Message.elements(of: collection)
        case .unavailable: return .value([])
        case .failed: return .failed
        }
    }

    public static func color(_ field: ReconColorField, of object: AnyObject)
        -> ReconRead<ReconColorFacts>
    {
        let binding = field.binding
        guard applies(binding, to: object) else { return .unavailable }
        guard field.isCoreGraphics else {
            switch Message.object(binding.selector, to: object) {
            case .value(let color): return Facts.color(of: color)
            case .unavailable: return .unavailable
            case .failed: return .failed
            }
        }
        switch Message.reference(binding.selector, to: object) {
        case .value(let reference): return coreGraphicsColor(reference)
        case .unavailable: return .unavailable
        case .failed: return .failed
        }
    }

    /// The font a control, text view, or cell carries.
    public static func carrierFont(of object: AnyObject) -> ReconRead<ReconFontFacts> {
        let carriers: [ReconNativeClass] = [.control, .textView, .cell]
        guard carriers.contains(where: { Classification.inherits(object, from: $0.objectiveCClass) })
        else { return .unavailable }
        switch Message.object(Fields.font, to: object) {
        case .value(let font): return Facts.font(of: font)
        case .unavailable: return .unavailable
        case .failed: return .failed
        }
    }

    /// The longest effective font run starting at `offset`. The caller bounds the walk.
    public static func fontRun(of attributedString: AnyObject, at offset: Int)
        -> ReconRead<ReconFontRun>
    {
        guard case .value(let length) = Message.integer(
            NSSelectorFromString("length"), to: attributedString)
        else { return .unavailable }
        guard offset < length else { return .unavailable }
        let key = NSAttributedString.Key.font.rawValue as NSString
        let read = Message.attribute(
            Fields.attributeAtIndex, to: attributedString, named: key, at: offset,
            within: NSRange(location: 0, length: length))
        switch read {
        case .value(let found):
            guard Self.describes(found.range, at: offset, within: length) else { return .failed }
            let font = found.value.map { Facts.font(of: $0) }
            if case .failed = font { return .failed }
            return .value(
                ReconFontRun(
                    location: found.range.location, length: found.range.length,
                    font: font?.value))
        case .unavailable: return .unavailable
        case .failed: return .failed
        }
    }

    /// A target decides what range its runs cover, so the answer is checked before the walk
    /// does arithmetic with it: a run must contain the index it was asked about and lie inside
    /// the string it came from.
    private static func describes(_ range: NSRange, at index: Int, within length: Int) -> Bool {
        guard range.location >= 0, range.length > 0, range.location <= index else { return false }
        let (end, overflowed) = range.location.addingReportingOverflow(range.length)
        return !overflowed && index < end && end <= length
    }

    public static func constraint(_ constraint: AnyObject) -> ReconRead<ReconConstraintFacts> {
        Facts.constraint(constraint)
    }

    public static func transform(ofLayer layer: AnyObject) -> ReconRead<[Double]> {
        guard Classification.inherits(layer, from: ReconNativeClass.layer.objectiveCClass) else {
            return .unavailable
        }
        return Message.transform(Fields.transform, to: layer).map { matrix in
            [
                matrix.m11, matrix.m12, matrix.m13, matrix.m14,
                matrix.m21, matrix.m22, matrix.m23, matrix.m24,
                matrix.m31, matrix.m32, matrix.m33, matrix.m34,
                matrix.m41, matrix.m42, matrix.m43, matrix.m44,
            ]
        }
    }

    public static func className(of object: AnyObject) -> String {
        Classification.name(of: object)
    }

    public static func superclassNames(of object: AnyObject, limit: Int) -> [String] {
        Classification.superclassNames(of: object, limit: limit)
    }

    public static func kind(of object: AnyObject) -> ReconObjectKind {
        Classification.kind(of: object)
    }

    public static func isKind(_ object: AnyObject, of nativeClass: ReconNativeClass) -> Bool {
        Classification.inherits(object, from: nativeClass.objectiveCClass)
    }

    private static func applies(_ binding: FieldBinding, to object: AnyObject) -> Bool {
        guard let owner = binding.owner else { return true }
        return Classification.inherits(object, from: owner.objectiveCClass)
    }

    private static func count(_ selector: Selector, of object: AnyObject) -> ReconRead<Int> {
        switch Message.object(selector, to: object) {
        case .value(let collection): Message.integer(Fields.count, to: collection)
        case .unavailable: .value(0)
        case .failed: .failed
        }
    }

    private static func screenNumber(of screen: AnyObject) -> ReconRead<Int> {
        switch Message.object(Fields.deviceDescription, to: screen) {
        case .value(let description):
            switch Message.object(
                Fields.objectForKey, to: description, with: "NSScreenNumber" as NSString)
            {
            case .value(let number): return Message.integer(Fields.integerValue, to: number)
            case .unavailable: return .unavailable
            case .failed: return .failed
            }
        case .unavailable: return .unavailable
        case .failed: return .failed
        }
    }

    private static func coreGraphicsColor(_ reference: UnsafeRawPointer)
        -> ReconRead<ReconColorFacts>
    {
        let color = Unmanaged<AnyObject>.fromOpaque(reference).takeUnretainedValue()
        switch Message.object(Fields.colorWithCGColor, to: NSColor.self, with: color) {
        case .value(let converted): return Facts.color(of: converted)
        case .unavailable: return .unavailable
        case .failed: return .failed
        }
    }
}
