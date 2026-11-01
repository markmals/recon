#import "ReconAccessorsInternal.h"

#import <objc/message.h>

static SEL selectorForField(ReconStringField field) {
    switch (field) {
        case ReconStringFieldTitle: return @selector(title);
        case ReconStringFieldToolTip: return @selector(toolTip);
        case ReconStringFieldIdentifier: return @selector(identifier);
        case ReconStringFieldAccessibilityIdentifier: return @selector(accessibilityIdentifier);
        case ReconStringFieldAccessibilityRole: return @selector(accessibilityRole);
        case ReconStringFieldLayerName: return @selector(name);
        case ReconStringFieldWindowAppearanceName: return @selector(appearance);
        case ReconStringFieldViewEffectiveAppearanceName: return @selector(effectiveAppearance);
        case ReconStringFieldConstraintIdentifier: return @selector(identifier);
    }
}

/// `-name` is shared by too many unrelated classes to read it off anything but a layer.
static BOOL fieldApplies(id object, ReconStringField field) {
    return field != ReconStringFieldLayerName || ReconIsKindOf(object, ReconNativeClassLayer);
}

static BOOL namesAnAppearance(ReconStringField field) {
    return field == ReconStringFieldWindowAppearanceName ||
           field == ReconStringFieldViewEffectiveAppearanceName;
}

static ReconReadStatus stringOutcome(id _Nullable read, NSString **value) {
    if (!read) return ReconReadStatusUnavailable;
    if (![read isKindOfClass:NSString.class]) return ReconReadStatusFailed;
    *value = read;
    return ReconReadStatusValue;
}

static ReconReadStatus readStringField(id object, ReconStringField field, NSString **value) {
    if (!fieldApplies(object, field)) return ReconReadStatusUnavailable;
    SEL selector = selectorForField(field);
    if (![object respondsToSelector:selector]) return ReconReadStatusUnavailable;
    id read = ((id (*)(id, SEL))objc_msgSend)(object, selector);
    if (!namesAnAppearance(field)) return stringOutcome(read, value);
    if (!read) return ReconReadStatusUnavailable;
    if (![read isKindOfClass:NSAppearance.class]) return ReconReadStatusFailed;
    return stringOutcome(((NSAppearance *)read).name, value);
}

ReconReadStatus ReconReadString(id object, ReconStringField field, NSString **value) {
    *value = nil;
    @try {
        return readStringField(object, field, value);
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

BOOL ReconRespondsToString(id object, ReconStringField field) {
    @try {
        return fieldApplies(object, field) && [object respondsToSelector:selectorForField(field)];
    } @catch (id exception) {
        return NO;
    }
}
