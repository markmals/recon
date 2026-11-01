#import "ReconAccessorsInternal.h"

static ReconReadStatus viewDouble(id object, ReconDoubleField field, double *value) {
    if (!ReconIsKindOf(object, ReconNativeClassView)) return ReconReadStatusUnavailable;
    NSView *view = object;
    switch (field) {
        case ReconDoubleFieldViewAlpha:
            *value = view.alphaValue;
            break;
        case ReconDoubleFieldViewHuggingHorizontal:
            *value = [view contentHuggingPriorityForOrientation:NSLayoutConstraintOrientationHorizontal];
            break;
        case ReconDoubleFieldViewHuggingVertical:
            *value = [view contentHuggingPriorityForOrientation:NSLayoutConstraintOrientationVertical];
            break;
        case ReconDoubleFieldViewCompressionHorizontal:
            *value = [view contentCompressionResistancePriorityForOrientation:NSLayoutConstraintOrientationHorizontal];
            break;
        case ReconDoubleFieldViewCompressionVertical:
            *value = [view contentCompressionResistancePriorityForOrientation:NSLayoutConstraintOrientationVertical];
            break;
        default:
            return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus windowDouble(id object, double *value) {
    if (!ReconIsKindOf(object, ReconNativeClassWindow)) return ReconReadStatusUnavailable;
    *value = [(NSWindow *)object backingScaleFactor];
    return ReconReadStatusValue;
}

static ReconReadStatus layerDouble(id object, ReconDoubleField field, double *value) {
    if (!ReconIsKindOf(object, ReconNativeClassLayer)) return ReconReadStatusUnavailable;
    CALayer *layer = object;
    switch (field) {
        case ReconDoubleFieldLayerOpacity: *value = layer.opacity; break;
        case ReconDoubleFieldLayerContentsScale: *value = layer.contentsScale; break;
        case ReconDoubleFieldLayerCornerRadius: *value = layer.cornerRadius; break;
        case ReconDoubleFieldLayerBorderWidth: *value = layer.borderWidth; break;
        case ReconDoubleFieldLayerShadowOpacity: *value = layer.shadowOpacity; break;
        case ReconDoubleFieldLayerShadowRadius: *value = layer.shadowRadius; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus constraintDouble(id object, ReconDoubleField field, double *value) {
    if (![object isKindOfClass:NSLayoutConstraint.class]) return ReconReadStatusUnavailable;
    NSLayoutConstraint *constraint = object;
    switch (field) {
        case ReconDoubleFieldConstraintMultiplier: *value = constraint.multiplier; break;
        case ReconDoubleFieldConstraintConstant: *value = constraint.constant; break;
        case ReconDoubleFieldConstraintPriority: *value = constraint.priority; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus readDoubleField(id object, ReconDoubleField field, double *value) {
    switch (field) {
        case ReconDoubleFieldViewAlpha:
        case ReconDoubleFieldViewHuggingHorizontal:
        case ReconDoubleFieldViewHuggingVertical:
        case ReconDoubleFieldViewCompressionHorizontal:
        case ReconDoubleFieldViewCompressionVertical: return viewDouble(object, field, value);
        case ReconDoubleFieldWindowBackingScaleFactor: return windowDouble(object, value);
        case ReconDoubleFieldLayerOpacity:
        case ReconDoubleFieldLayerContentsScale:
        case ReconDoubleFieldLayerCornerRadius:
        case ReconDoubleFieldLayerBorderWidth:
        case ReconDoubleFieldLayerShadowOpacity:
        case ReconDoubleFieldLayerShadowRadius: return layerDouble(object, field, value);
        case ReconDoubleFieldConstraintMultiplier:
        case ReconDoubleFieldConstraintConstant:
        case ReconDoubleFieldConstraintPriority: return constraintDouble(object, field, value);
    }
}

ReconReadStatus ReconReadDouble(id object, ReconDoubleField field, double *value) {
    *value = 0;
    @try {
        return readDoubleField(object, field, value);
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}
