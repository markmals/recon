#import "ReconAccessorsInternal.h"

static ReconReadStatus found(id _Nullable read, id *value) {
    *value = read;
    return read ? ReconReadStatusValue : ReconReadStatusUnavailable;
}

static ReconReadStatus windowObject(id object, ReconObjectField field, id *value) {
    if (!ReconIsKindOf(object, ReconNativeClassWindow)) return ReconReadStatusUnavailable;
    NSWindow *window = object;
    switch (field) {
        case ReconObjectFieldWindowContentView: return found(window.contentView, value);
        case ReconObjectFieldWindowAttachedSheet: return found(window.attachedSheet, value);
        case ReconObjectFieldWindowParentWindow: return found(window.parentWindow, value);
        case ReconObjectFieldWindowScreen: return found(window.screen, value);
        default: return ReconReadStatusUnavailable;
    }
}

static ReconReadStatus viewObject(id object, ReconObjectField field, id *value) {
    if (!ReconIsKindOf(object, ReconNativeClassView)) return ReconReadStatusUnavailable;
    NSView *view = object;
    switch (field) {
        case ReconObjectFieldViewSuperview: return found(view.superview, value);
        case ReconObjectFieldViewWindow: return found(view.window, value);
        case ReconObjectFieldViewLayer: return found(view.layer, value);
        default: return ReconReadStatusUnavailable;
    }
}

static ReconReadStatus layerObject(id object, ReconObjectField field, id *value) {
    if (!ReconIsKindOf(object, ReconNativeClassLayer)) return ReconReadStatusUnavailable;
    CALayer *layer = object;
    switch (field) {
        case ReconObjectFieldLayerSuperlayer: return found(layer.superlayer, value);
        case ReconObjectFieldLayerMask: return found(layer.mask, value);
        case ReconObjectFieldLayerDelegate: return found(layer.delegate, value);
        default: return ReconReadStatusUnavailable;
    }
}

static ReconReadStatus guideObject(id object, id *value) {
    if (!ReconIsKindOf(object, ReconNativeClassLayoutGuide)) return ReconReadStatusUnavailable;
    return found([(NSLayoutGuide *)object owningView], value);
}

static ReconReadStatus controlCell(id object, id *value) {
    if (!ReconIsKindOf(object, ReconNativeClassControl)) return ReconReadStatusUnavailable;
    return found([(NSControl *)object cell], value);
}

static ReconReadStatus attributedStringValue(id object, id *value) {
    if (ReconIsKindOf(object, ReconNativeClassControl)) {
        return found([(NSControl *)object attributedStringValue], value);
    }
    if (ReconIsKindOf(object, ReconNativeClassCell)) {
        return found([(NSCell *)object attributedStringValue], value);
    }
    return ReconReadStatusUnavailable;
}

static ReconReadStatus readObjectField(id object, ReconObjectField field, id *value) {
    switch (field) {
        case ReconObjectFieldWindowContentView:
        case ReconObjectFieldWindowAttachedSheet:
        case ReconObjectFieldWindowParentWindow:
        case ReconObjectFieldWindowScreen: return windowObject(object, field, value);
        case ReconObjectFieldViewSuperview:
        case ReconObjectFieldViewWindow:
        case ReconObjectFieldViewLayer: return viewObject(object, field, value);
        case ReconObjectFieldLayerSuperlayer:
        case ReconObjectFieldLayerMask:
        case ReconObjectFieldLayerDelegate: return layerObject(object, field, value);
        case ReconObjectFieldGuideOwningView: return guideObject(object, value);
        case ReconObjectFieldControlCell: return controlCell(object, value);
        case ReconObjectFieldAttributedStringValue: return attributedStringValue(object, value);
    }
}

ReconReadStatus ReconReadObject(id object, ReconObjectField field, id *value) {
    *value = nil;
    @try {
        return readObjectField(object, field, value);
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

static ReconReadStatus viewArray(id object, ReconArrayField field, NSArray **value) {
    if (!ReconIsKindOf(object, ReconNativeClassView)) return ReconReadStatusUnavailable;
    NSView *view = object;
    switch (field) {
        case ReconArrayFieldViewSubviews: *value = [view.subviews copy]; break;
        case ReconArrayFieldViewConstraints: *value = [view.constraints copy]; break;
        case ReconArrayFieldViewLayoutGuides: *value = [view.layoutGuides copy]; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

/// A window with no child windows and a layer with no sublayers answer nil, which is no children
/// rather than an unreadable list.
static ReconReadStatus windowArray(id object, NSArray **value) {
    if (!ReconIsKindOf(object, ReconNativeClassWindow)) return ReconReadStatusUnavailable;
    *value = [[(NSWindow *)object childWindows] copy] ?: @[];
    return ReconReadStatusValue;
}

static ReconReadStatus layerArray(id object, NSArray **value) {
    if (!ReconIsKindOf(object, ReconNativeClassLayer)) return ReconReadStatusUnavailable;
    *value = [[(CALayer *)object sublayers] copy] ?: @[];
    return ReconReadStatusValue;
}

static ReconReadStatus readArrayField(id object, ReconArrayField field, NSArray **value) {
    switch (field) {
        case ReconArrayFieldViewSubviews:
        case ReconArrayFieldViewConstraints:
        case ReconArrayFieldViewLayoutGuides: return viewArray(object, field, value);
        case ReconArrayFieldWindowChildWindows: return windowArray(object, value);
        case ReconArrayFieldLayerSublayers: return layerArray(object, value);
    }
}

ReconReadStatus ReconReadArray(id object, ReconArrayField field, NSArray **value) {
    *value = nil;
    @try {
        return readArrayField(object, field, value);
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}
