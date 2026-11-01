import CoreGraphics
import ReconNativeSupport

/// Typed reads of one live object, with a raising getter failing the facet that asked for it.
///
/// `nil` is the object not having the value; it is never a getter that raised, which is what
/// keeps "absent" and "failed" distinct in every published observation.
extension Work {
    func text(_ field: ReconStringField, of object: AnyObject) throws -> String? {
        try value(ReconNative.string(field, of: object)).map(bounded)
    }

    func flag(_ field: ReconBoolField, of object: AnyObject) throws -> Bool? {
        try value(ReconNative.bool(field, of: object))
    }

    func integer(_ field: ReconIntegerField, of object: AnyObject) throws -> Int? {
        try value(ReconNative.integer(field, of: object))
    }

    func number(_ field: ReconDoubleField, of object: AnyObject) throws -> Double? {
        try value(ReconNative.double(field, of: object))
    }

    func rect(_ field: ReconRectField, of object: AnyObject) throws -> CGRect? {
        try value(ReconNative.rect(field, of: object))
    }

    func related(_ field: ReconObjectField, of object: AnyObject) throws -> AnyObject? {
        try value(ReconNative.object(field, of: object))
    }

    func list(_ field: ReconArrayField, of object: AnyObject) throws -> [AnyObject] {
        try value(ReconNative.array(field, of: object)) ?? []
    }

    func value<T>(_ read: ReconRead<T>) throws -> T? {
        switch read {
        case .value(let value): value
        case .unavailable: nil
        case .failed: throw TargetGetterRaised()
        }
    }

    /// A handle for a related object, or the status that explains why there is none.
    func reference(_ field: ReconObjectField, of object: AnyObject) throws -> Reference {
        Reference(reference(try related(field, of: object)))
    }

    /// The accessibility interface's own property, read in the target through an overridable
    /// getter that runs application code. It is not `NSView.identifier`, which is a different
    /// property with a different purpose.
    func accessibility(_ field: ReconStringField, of object: AnyObject) throws -> AccessibilityValue
    {
        guard ReconNative.responds(to: field, of: object) else { return .unsupported }
        return AccessibilityValue(try text(field, of: object))
    }
}
