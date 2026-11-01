#import "ReconAccessorsInternal.h"

static ReconReadStatus windowBool(id object, ReconBoolField field, BOOL *value) {
    if (!ReconIsKindOf(object, ReconNativeClassWindow)) return ReconReadStatusUnavailable;
    NSWindow *window = object;
    switch (field) {
        case ReconBoolFieldWindowVisible: *value = window.isVisible; break;
        case ReconBoolFieldWindowKey: *value = window.isKeyWindow; break;
        case ReconBoolFieldWindowMain: *value = window.isMainWindow; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus viewBool(id object, ReconBoolField field, BOOL *value) {
    if (!ReconIsKindOf(object, ReconNativeClassView)) return ReconReadStatusUnavailable;
    NSView *view = object;
    switch (field) {
        case ReconBoolFieldViewFlipped: *value = view.isFlipped; break;
        case ReconBoolFieldViewHidden: *value = view.isHidden; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus layerBool(id object, ReconBoolField field, BOOL *value) {
    if (!ReconIsKindOf(object, ReconNativeClassLayer)) return ReconReadStatusUnavailable;
    CALayer *layer = object;
    switch (field) {
        case ReconBoolFieldLayerHidden: *value = layer.isHidden; break;
        case ReconBoolFieldLayerMasksToBounds: *value = layer.masksToBounds; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus cellBool(id object, ReconBoolField field, BOOL *value) {
    if (!ReconIsKindOf(object, ReconNativeClassCell)) return ReconReadStatusUnavailable;
    NSCell *cell = object;
    switch (field) {
        case ReconBoolFieldCellEnabled: *value = cell.isEnabled; break;
        case ReconBoolFieldCellEditable: *value = cell.isEditable; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus constraintBool(id object, BOOL *value) {
    if (![object isKindOfClass:NSLayoutConstraint.class]) return ReconReadStatusUnavailable;
    *value = [(NSLayoutConstraint *)object isActive];
    return ReconReadStatusValue;
}

static ReconReadStatus visualEffectBool(id object, BOOL *value) {
    if (!ReconIsKindOf(object, ReconNativeClassVisualEffectView)) return ReconReadStatusUnavailable;
    *value = [(NSVisualEffectView *)object isEmphasized];
    return ReconReadStatusValue;
}

static ReconReadStatus readBoolField(id object, ReconBoolField field, BOOL *value) {
    switch (field) {
        case ReconBoolFieldWindowVisible:
        case ReconBoolFieldWindowKey:
        case ReconBoolFieldWindowMain: return windowBool(object, field, value);
        case ReconBoolFieldViewFlipped:
        case ReconBoolFieldViewHidden: return viewBool(object, field, value);
        case ReconBoolFieldLayerHidden:
        case ReconBoolFieldLayerMasksToBounds: return layerBool(object, field, value);
        case ReconBoolFieldCellEnabled:
        case ReconBoolFieldCellEditable: return cellBool(object, field, value);
        case ReconBoolFieldConstraintActive: return constraintBool(object, value);
        case ReconBoolFieldVisualEffectEmphasized: return visualEffectBool(object, value);
    }
}

ReconReadStatus ReconReadBool(id object, ReconBoolField field, BOOL *value) {
    *value = NO;
    @try {
        return readBoolField(object, field, value);
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}
