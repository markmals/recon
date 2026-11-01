import CoreGraphics
import Foundation

// The overlay is the whole surface: an author imports one module and names the allowlisted
// fields through it.
@_exported import ReconNativeAccessors

/// The allowlisted way to read a live target object from Swift.
///
/// Every function here calls into Objective-C, which invokes the getter and catches any
/// Objective-C exception before control returns. There is no accessor that takes a selector, a
/// key path, or an expression, and a client can never supply one; adding a family is a change to
/// native support, not something an extension does for itself.
public enum ReconNative {
    public static func string(_ field: ReconStringField, of object: AnyObject) -> ReconRead<String> {
        var value: NSString?
        return read(ReconReadString(object, field, &value)) { value as String? }
    }

    /// Whether the object implements the getter at all. An observation reports a getter the
    /// object does not implement as unsupported, which is not the same as having no value.
    public static func responds(to field: ReconStringField, of object: AnyObject) -> Bool {
        ReconRespondsToString(object, field)
    }

    public static func bool(_ field: ReconBoolField, of object: AnyObject) -> ReconRead<Bool> {
        var value = ObjCBool(false)
        return read(ReconReadBool(object, field, &value)) { value.boolValue }
    }

    public static func integer(_ field: ReconIntegerField, of object: AnyObject) -> ReconRead<Int> {
        var value = 0
        return read(ReconReadInteger(object, field, &value)) { value }
    }

    public static func double(_ field: ReconDoubleField, of object: AnyObject) -> ReconRead<Double> {
        var value = 0.0
        return read(ReconReadDouble(object, field, &value)) { value }
    }

    public static func rect(_ field: ReconRectField, of object: AnyObject) -> ReconRead<CGRect> {
        var value = CGRect.zero
        return read(ReconReadRect(object, field, &value)) { value }
    }

    public static func size(_ field: ReconSizeField, of object: AnyObject) -> ReconRead<CGSize> {
        var value = CGSize.zero
        return read(ReconReadSize(object, field, &value)) { value }
    }

    public static func object(_ field: ReconObjectField, of object: AnyObject)
        -> ReconRead<AnyObject>
    {
        var value: AnyObject?
        return read(ReconReadObject(object, field, &value)) { value }
    }

    public static func array(_ field: ReconArrayField, of object: AnyObject)
        -> ReconRead<[AnyObject]>
    {
        var value: NSArray?
        return read(ReconReadArray(object, field, &value)) { value as? [AnyObject] }
    }

    public static func color(_ field: ReconColorField, of object: AnyObject)
        -> ReconRead<ReconColorFacts>
    {
        var value: ReconColorFacts?
        return read(ReconReadColor(object, field, &value)) { value }
    }

    /// The font a control, text view, or cell carries.
    public static func carrierFont(of object: AnyObject) -> ReconRead<ReconFontFacts> {
        var value: ReconFontFacts?
        return read(ReconReadCarrierFont(object, &value)) { value }
    }

    /// The longest effective font run starting at `offset`. The caller bounds the walk.
    public static func fontRun(of attributedString: AnyObject, at offset: Int)
        -> ReconRead<ReconFontRun>
    {
        var value: ReconFontRun?
        return read(ReconReadFontRun(attributedString, UInt(offset), &value)) { value }
    }

    public static func constraint(_ constraint: AnyObject) -> ReconRead<ReconConstraintFacts> {
        var value: ReconConstraintFacts?
        return read(ReconReadConstraint(constraint, &value)) { value }
    }

    public static func transform(ofLayer layer: AnyObject) -> ReconRead<[Double]> {
        var value: ReconTransformFacts?
        return read(ReconReadLayerTransform(layer, &value)) { value?.matrix.map(\.doubleValue) }
    }

    /// `view.bounds` in its window's coordinates.
    public static func rectInWindow(of view: AnyObject) -> ReconRead<CGRect> {
        var value = CGRect.zero
        return read(ReconReadViewRectInWindow(view, &value)) { value }
    }

    public static func screenRect(of window: AnyObject, _ rect: CGRect) -> ReconRead<CGRect> {
        var value = CGRect.zero
        return read(ReconConvertWindowRectToScreen(window, rect, &value)) { value }
    }

    public static func className(of object: AnyObject) -> String { ReconClassName(object) }

    public static func superclassNames(of object: AnyObject, limit: Int) -> [String] {
        ReconSuperclassNames(object, UInt(limit))
    }

    public static func kind(of object: AnyObject) -> ReconObjectKind { ReconKindOf(object) }

    public static func isKind(_ object: AnyObject, of nativeClass: ReconNativeClass) -> Bool {
        ReconIsKindOf(object, nativeClass)
    }

    public static func applicationWindows() -> ReconRead<[AnyObject]> {
        var value: NSArray?
        return read(ReconReadApplicationWindows(&value)) { value as? [AnyObject] }
    }

    public static func applicationWindow(number: Int) -> AnyObject? {
        ReconApplicationWindowWithNumber(number) as AnyObject?
    }

    public static func windowServerInventory() -> [ReconWindowServerEntry] {
        ReconWindowServerInventory()
    }

    /// The height of the screen that owns the global origin, or NaN where none reports one.
    public static var primaryScreenHeight: Double { ReconPrimaryScreenHeight() }

    public static func screenTopLeft(_ rect: CGRect, primaryScreenHeight: Double) -> CGRect {
        ReconScreenTopLeftRect(rect, primaryScreenHeight)
    }

    /// A value present but malformed is a failed read, not an absent one.
    private static func read<Value>(_ status: ReconReadStatus, _ value: () -> Value?)
        -> ReconRead<Value>
    {
        switch status {
        case .value: value().map { ReconRead.value($0) } ?? .failed
        case .unavailable: .unavailable
        default: .failed
        }
    }
}
