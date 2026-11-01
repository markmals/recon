#import "ReconAccessorsInternal.h"

static ReconReadStatus windowInteger(id object, NSInteger *value) {
    if (!ReconIsKindOf(object, ReconNativeClassWindow)) return ReconReadStatusUnavailable;
    *value = [(NSWindow *)object windowNumber];
    return ReconReadStatusValue;
}

static ReconReadStatus viewInteger(id object, NSInteger *value) {
    if (!ReconIsKindOf(object, ReconNativeClassView)) return ReconReadStatusUnavailable;
    *value = (NSInteger)[(NSView *)object subviews].count;
    return ReconReadStatusValue;
}

static ReconReadStatus cellInteger(id object, ReconIntegerField field, NSInteger *value) {
    if (!ReconIsKindOf(object, ReconNativeClassCell)) return ReconReadStatusUnavailable;
    NSCell *cell = object;
    switch (field) {
        case ReconIntegerFieldCellType: *value = (NSInteger)cell.type; break;
        case ReconIntegerFieldCellState: *value = cell.state; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus layerInteger(id object, ReconIntegerField field, NSInteger *value) {
    if (!ReconIsKindOf(object, ReconNativeClassLayer)) return ReconReadStatusUnavailable;
    CALayer *layer = object;
    switch (field) {
        case ReconIntegerFieldLayerSublayerCount: *value = (NSInteger)layer.sublayers.count; break;
        case ReconIntegerFieldLayerMaskedCorners: *value = (NSInteger)layer.maskedCorners; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus visualEffectInteger(id object, ReconIntegerField field, NSInteger *value) {
    if (!ReconIsKindOf(object, ReconNativeClassVisualEffectView)) return ReconReadStatusUnavailable;
    NSVisualEffectView *effect = object;
    switch (field) {
        case ReconIntegerFieldVisualEffectMaterial: *value = effect.material; break;
        case ReconIntegerFieldVisualEffectBlendingMode: *value = effect.blendingMode; break;
        case ReconIntegerFieldVisualEffectState: *value = effect.state; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus constraintInteger(id object, ReconIntegerField field, NSInteger *value) {
    if (![object isKindOfClass:NSLayoutConstraint.class]) return ReconReadStatusUnavailable;
    NSLayoutConstraint *constraint = object;
    switch (field) {
        case ReconIntegerFieldConstraintFirstAttribute: *value = constraint.firstAttribute; break;
        case ReconIntegerFieldConstraintSecondAttribute: *value = constraint.secondAttribute; break;
        case ReconIntegerFieldConstraintRelation: *value = constraint.relation; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus attributedStringInteger(id object, NSInteger *value) {
    if (![object isKindOfClass:NSAttributedString.class]) return ReconReadStatusUnavailable;
    *value = (NSInteger)[(NSAttributedString *)object length];
    return ReconReadStatusValue;
}

static ReconReadStatus screenInteger(id object, NSInteger *value) {
    if (![object isKindOfClass:NSScreen.class]) return ReconReadStatusUnavailable;
    id number = [(NSScreen *)object deviceDescription][@"NSScreenNumber"];
    if (![number isKindOfClass:NSNumber.class]) return ReconReadStatusUnavailable;
    *value = [number integerValue];
    return ReconReadStatusValue;
}

static ReconReadStatus readIntegerField(id object, ReconIntegerField field, NSInteger *value) {
    switch (field) {
        case ReconIntegerFieldWindowNumber: return windowInteger(object, value);
        case ReconIntegerFieldCellType:
        case ReconIntegerFieldCellState: return cellInteger(object, field, value);
        case ReconIntegerFieldViewSubviewCount: return viewInteger(object, value);
        case ReconIntegerFieldLayerSublayerCount:
        case ReconIntegerFieldLayerMaskedCorners: return layerInteger(object, field, value);
        case ReconIntegerFieldVisualEffectMaterial:
        case ReconIntegerFieldVisualEffectBlendingMode:
        case ReconIntegerFieldVisualEffectState: return visualEffectInteger(object, field, value);
        case ReconIntegerFieldConstraintFirstAttribute:
        case ReconIntegerFieldConstraintSecondAttribute:
        case ReconIntegerFieldConstraintRelation: return constraintInteger(object, field, value);
        case ReconIntegerFieldAttributedStringLength: return attributedStringInteger(object, value);
        case ReconIntegerFieldScreenNumber: return screenInteger(object, value);
    }
}

ReconReadStatus ReconReadInteger(id object, ReconIntegerField field, NSInteger *value) {
    *value = 0;
    @try {
        return readIntegerField(object, field, value);
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}
