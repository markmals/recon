import AppKit
import Foundation

public enum ReconStringField: Sendable {
    case title
    case toolTip
    case identifier
    case accessibilityIdentifier
    case accessibilityRole
    case layerName
    case windowAppearanceName
    case viewEffectiveAppearanceName
    case constraintIdentifier

    var binding: FieldBinding {
        switch self {
        case .title: FieldBinding(NSSelectorFromString("title"))
        case .toolTip: FieldBinding(NSSelectorFromString("toolTip"))
        case .identifier: FieldBinding(Fields.identifier)
        case .accessibilityIdentifier:
            FieldBinding(NSSelectorFromString("accessibilityIdentifier"))
        case .accessibilityRole: FieldBinding(NSSelectorFromString("accessibilityRole"))
        // `-name` is shared by too many unrelated classes to read it off anything but a layer.
        case .layerName: FieldBinding(Fields.name, .layer)
        case .windowAppearanceName: FieldBinding(NSSelectorFromString("appearance"), .window)
        case .viewEffectiveAppearanceName:
            FieldBinding(NSSelectorFromString("effectiveAppearance"), .view)
        case .constraintIdentifier: FieldBinding(Fields.identifier)
        }
    }

    /// The appearance fields name an appearance object, whose own name is the published value.
    var readsAnAppearance: Bool {
        self == .windowAppearanceName || self == .viewEffectiveAppearanceName
    }
}

public enum ReconBoolField: Sendable {
    case windowVisible, windowKey, windowMain
    case viewFlipped, viewHidden
    case layerHidden, layerMasksToBounds
    case cellEnabled, cellEditable
    case constraintActive
    case visualEffectEmphasized

    var binding: FieldBinding {
        switch self {
        case .windowVisible: FieldBinding(NSSelectorFromString("isVisible"), .window)
        case .windowKey: FieldBinding(NSSelectorFromString("isKeyWindow"), .window)
        case .windowMain: FieldBinding(NSSelectorFromString("isMainWindow"), .window)
        case .viewFlipped: FieldBinding(NSSelectorFromString("isFlipped"), .view)
        case .viewHidden: FieldBinding(NSSelectorFromString("isHidden"), .view)
        case .layerHidden: FieldBinding(NSSelectorFromString("isHidden"), .layer)
        case .layerMasksToBounds: FieldBinding(NSSelectorFromString("masksToBounds"), .layer)
        case .cellEnabled: FieldBinding(NSSelectorFromString("isEnabled"), .cell)
        case .cellEditable: FieldBinding(NSSelectorFromString("isEditable"), .cell)
        case .constraintActive: FieldBinding(Fields.isActive)
        case .visualEffectEmphasized:
            FieldBinding(NSSelectorFromString("isEmphasized"), .visualEffectView)
        }
    }
}

public enum ReconIntegerField: Sendable {
    case windowNumber
    case cellType, cellState
    case viewSubviewCount, layerSublayerCount
    case layerMaskedCorners
    case visualEffectMaterial, visualEffectBlendingMode, visualEffectState
    case constraintFirstAttribute, constraintSecondAttribute, constraintRelation
    case attributedStringLength
    case screenNumber

    var binding: FieldBinding {
        switch self {
        case .windowNumber: FieldBinding(NSSelectorFromString("windowNumber"), .window)
        case .cellType: FieldBinding(NSSelectorFromString("type"), .cell)
        case .cellState: FieldBinding(NSSelectorFromString("state"), .cell)
        case .viewSubviewCount: FieldBinding(NSSelectorFromString("subviews"), .view)
        case .layerSublayerCount: FieldBinding(NSSelectorFromString("sublayers"), .layer)
        case .layerMaskedCorners: FieldBinding(NSSelectorFromString("maskedCorners"), .layer)
        case .visualEffectMaterial:
            FieldBinding(NSSelectorFromString("material"), .visualEffectView)
        case .visualEffectBlendingMode:
            FieldBinding(NSSelectorFromString("blendingMode"), .visualEffectView)
        case .visualEffectState: FieldBinding(NSSelectorFromString("state"), .visualEffectView)
        case .constraintFirstAttribute: FieldBinding(Fields.firstAttribute)
        case .constraintSecondAttribute: FieldBinding(Fields.secondAttribute)
        case .constraintRelation: FieldBinding(Fields.relation)
        case .attributedStringLength: FieldBinding(NSSelectorFromString("length"))
        case .screenNumber: FieldBinding(Fields.deviceDescription)
        }
    }
}

/// Whether a getter answers a single- or double-precision value, which decides how it is sent.
enum Precision {
    case single, double
}

public enum ReconDoubleField: Sendable {
    case viewAlpha
    case windowBackingScaleFactor
    case layerOpacity, layerContentsScale, layerCornerRadius, layerBorderWidth
    case layerShadowOpacity, layerShadowRadius
    case viewHuggingHorizontal, viewHuggingVertical
    case viewCompressionHorizontal, viewCompressionVertical
    case constraintMultiplier, constraintConstant, constraintPriority

    var binding: FieldBinding {
        switch self {
        case .viewAlpha: FieldBinding(NSSelectorFromString("alphaValue"), .view)
        case .windowBackingScaleFactor:
            FieldBinding(NSSelectorFromString("backingScaleFactor"), .window)
        case .layerOpacity: FieldBinding(NSSelectorFromString("opacity"), .layer)
        case .layerContentsScale: FieldBinding(NSSelectorFromString("contentsScale"), .layer)
        case .layerCornerRadius: FieldBinding(NSSelectorFromString("cornerRadius"), .layer)
        case .layerBorderWidth: FieldBinding(NSSelectorFromString("borderWidth"), .layer)
        case .layerShadowOpacity: FieldBinding(NSSelectorFromString("shadowOpacity"), .layer)
        case .layerShadowRadius: FieldBinding(NSSelectorFromString("shadowRadius"), .layer)
        case .viewHuggingHorizontal, .viewHuggingVertical:
            FieldBinding(NSSelectorFromString("contentHuggingPriorityForOrientation:"), .view)
        case .viewCompressionHorizontal, .viewCompressionVertical:
            FieldBinding(
                NSSelectorFromString("contentCompressionResistancePriorityForOrientation:"), .view)
        case .constraintMultiplier: FieldBinding(Fields.multiplier)
        case .constraintConstant: FieldBinding(Fields.constant)
        case .constraintPriority: FieldBinding(Fields.priority)
        }
    }

    var precision: Precision {
        switch self {
        case .layerOpacity, .layerShadowOpacity, .constraintPriority, .viewHuggingHorizontal,
            .viewHuggingVertical, .viewCompressionHorizontal, .viewCompressionVertical:
            .single
        default:
            .double
        }
    }

    /// The layout orientation a priority is asked for, where the getter takes one.
    var orientation: Int? {
        switch self {
        case .viewHuggingHorizontal, .viewCompressionHorizontal: 0
        case .viewHuggingVertical, .viewCompressionVertical: 1
        default: nil
        }
    }
}
