import AppKit
import Foundation

public enum ReconRectField: Sendable {
    case windowFrame, viewFrame, viewBounds, layerFrame, layerBounds, guideFrame

    var binding: FieldBinding {
        switch self {
        case .windowFrame: FieldBinding(Fields.frame, .window)
        case .viewFrame: FieldBinding(Fields.frame, .view)
        case .viewBounds: FieldBinding(Fields.bounds, .view)
        case .layerFrame: FieldBinding(Fields.frame, .layer)
        case .layerBounds: FieldBinding(Fields.bounds, .layer)
        case .guideFrame: FieldBinding(Fields.frame, .layoutGuide)
        }
    }
}

public enum ReconSizeField: Sendable {
    case viewIntrinsicContentSize

    var binding: FieldBinding {
        FieldBinding(NSSelectorFromString("intrinsicContentSize"), .view)
    }
}

public enum ReconObjectField: Sendable {
    case windowContentView, windowAttachedSheet, windowParentWindow, windowScreen
    case viewSuperview, viewWindow, viewLayer
    case layerSuperlayer, layerMask, layerDelegate
    case guideOwningView
    case controlCell
    case attributedStringValue

    var binding: FieldBinding {
        switch self {
        case .windowContentView: FieldBinding(NSSelectorFromString("contentView"), .window)
        case .windowAttachedSheet: FieldBinding(NSSelectorFromString("attachedSheet"), .window)
        case .windowParentWindow: FieldBinding(NSSelectorFromString("parentWindow"), .window)
        case .windowScreen: FieldBinding(NSSelectorFromString("screen"), .window)
        case .viewSuperview: FieldBinding(NSSelectorFromString("superview"), .view)
        case .viewWindow: FieldBinding(Fields.window, .view)
        case .viewLayer: FieldBinding(NSSelectorFromString("layer"), .view)
        case .layerSuperlayer: FieldBinding(NSSelectorFromString("superlayer"), .layer)
        case .layerMask: FieldBinding(NSSelectorFromString("mask"), .layer)
        case .layerDelegate: FieldBinding(NSSelectorFromString("delegate"), .layer)
        case .guideOwningView: FieldBinding(NSSelectorFromString("owningView"), .layoutGuide)
        case .controlCell: FieldBinding(NSSelectorFromString("cell"), .control)
        case .attributedStringValue:
            FieldBinding(NSSelectorFromString("attributedStringValue"))
        }
    }

    /// An attributed value belongs to a control or to a cell, and to nothing else.
    var carriers: [ReconNativeClass] {
        self == .attributedStringValue ? [.control, .cell] : []
    }
}

public enum ReconArrayField: Sendable {
    case viewSubviews, viewConstraints, viewLayoutGuides, windowChildWindows, layerSublayers

    var binding: FieldBinding {
        switch self {
        case .viewSubviews: FieldBinding(NSSelectorFromString("subviews"), .view)
        case .viewConstraints: FieldBinding(NSSelectorFromString("constraints"), .view)
        case .viewLayoutGuides: FieldBinding(NSSelectorFromString("layoutGuides"), .view)
        case .windowChildWindows: FieldBinding(NSSelectorFromString("childWindows"), .window)
        case .layerSublayers: FieldBinding(NSSelectorFromString("sublayers"), .layer)
        }
    }
}

public enum ReconColorField: Sendable {
    case windowBackgroundColor
    case textFieldTextColor, textFieldBackgroundColor, textFieldCellTextColor
    case boxFillColor
    case layerBackgroundColor, layerBorderColor, layerShadowColor

    var binding: FieldBinding {
        switch self {
        case .windowBackgroundColor:
            FieldBinding(NSSelectorFromString("backgroundColor"), .window)
        case .textFieldTextColor: FieldBinding(NSSelectorFromString("textColor"), .textField)
        case .textFieldBackgroundColor:
            FieldBinding(NSSelectorFromString("backgroundColor"), .textField)
        case .textFieldCellTextColor:
            FieldBinding(NSSelectorFromString("textColor"), .textFieldCell)
        case .boxFillColor: FieldBinding(NSSelectorFromString("fillColor"), .box)
        case .layerBackgroundColor:
            FieldBinding(NSSelectorFromString("backgroundColor"), .layer)
        case .layerBorderColor: FieldBinding(NSSelectorFromString("borderColor"), .layer)
        case .layerShadowColor: FieldBinding(NSSelectorFromString("shadowColor"), .layer)
        }
    }

    /// A layer's colours are Core Graphics colours, which become AppKit ones to be described.
    var isCoreGraphics: Bool {
        switch self {
        case .layerBackgroundColor, .layerBorderColor, .layerShadowColor: true
        default: false
        }
    }
}
