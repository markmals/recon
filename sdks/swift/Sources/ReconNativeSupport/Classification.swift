import AppKit
import ObjectiveC
import QuartzCore

/// The live classes a facet distinguishes. A client never names one.
public enum ReconNativeClass: Sendable {
    case window, view, layer, cell, layoutGuide
    case control, textView, textField, textFieldCell, box, visualEffectView

    var objectiveCClass: AnyClass {
        switch self {
        case .window: NSWindow.self
        case .view: NSView.self
        case .layer: CALayer.self
        case .cell: NSCell.self
        case .layoutGuide: NSLayoutGuide.self
        case .control: NSControl.self
        case .textView: NSTextView.self
        case .textField: NSTextField.self
        case .textFieldCell: NSTextFieldCell.self
        case .box: NSBox.self
        case .visualEffectView: NSVisualEffectView.self
        }
    }
}

/// The five object kinds Recon publishes handles for, and nothing else.
public enum ReconObjectKind: Sendable {
    case unsupported, window, view, layer, cell, layoutGuide
}

/// What an object is, decided from the runtime's class chain.
///
/// Classification sends nothing: `-isKindOfClass:` is a message an application may override and
/// may raise, and every caller needs to classify an object before deciding what to read from
/// it. Walking the chain the runtime already holds answers the same question without asking the
/// object anything.
enum Classification {
    static func inherits(_ object: AnyObject, from ancestor: AnyClass) -> Bool {
        var current: AnyClass? = object_getClass(object)
        while let candidate = current {
            if candidate === ancestor { return true }
            current = class_getSuperclass(candidate)
        }
        return false
    }

    static func kind(of object: AnyObject) -> ReconObjectKind {
        if inherits(object, from: NSWindow.self) { return .window }
        if inherits(object, from: NSView.self) { return .view }
        if inherits(object, from: CALayer.self) { return .layer }
        if inherits(object, from: NSCell.self) { return .cell }
        if inherits(object, from: NSLayoutGuide.self) { return .layoutGuide }
        return .unsupported
    }

    static func name(of object: AnyObject) -> String {
        NSStringFromClass(object_getClass(object) ?? type(of: object))
    }

    static func superclassNames(of object: AnyObject, limit: Int) -> [String] {
        var names: [String] = []
        var current: AnyClass? = class_getSuperclass(object_getClass(object) ?? type(of: object))
        while let candidate = current, names.count < limit {
            names.append(NSStringFromClass(candidate))
            current = class_getSuperclass(candidate)
        }
        return names
    }
}
