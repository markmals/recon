#import "ReconAccessorsInternal.h"

#import <math.h>

static ReconReadStatus windowRect(id object, CGRect *value) {
    if (!ReconIsKindOf(object, ReconNativeClassWindow)) return ReconReadStatusUnavailable;
    *value = [(NSWindow *)object frame];
    return ReconReadStatusValue;
}

static ReconReadStatus viewRect(id object, ReconRectField field, CGRect *value) {
    if (!ReconIsKindOf(object, ReconNativeClassView)) return ReconReadStatusUnavailable;
    NSView *view = object;
    switch (field) {
        case ReconRectFieldViewFrame: *value = view.frame; break;
        case ReconRectFieldViewBounds: *value = view.bounds; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus layerRect(id object, ReconRectField field, CGRect *value) {
    if (!ReconIsKindOf(object, ReconNativeClassLayer)) return ReconReadStatusUnavailable;
    CALayer *layer = object;
    switch (field) {
        case ReconRectFieldLayerFrame: *value = layer.frame; break;
        case ReconRectFieldLayerBounds: *value = layer.bounds; break;
        default: return ReconReadStatusUnavailable;
    }
    return ReconReadStatusValue;
}

static ReconReadStatus guideRect(id object, CGRect *value) {
    if (!ReconIsKindOf(object, ReconNativeClassLayoutGuide)) return ReconReadStatusUnavailable;
    *value = [(NSLayoutGuide *)object frame];
    return ReconReadStatusValue;
}

static ReconReadStatus readRectField(id object, ReconRectField field, CGRect *value) {
    switch (field) {
        case ReconRectFieldWindowFrame: return windowRect(object, value);
        case ReconRectFieldViewFrame:
        case ReconRectFieldViewBounds: return viewRect(object, field, value);
        case ReconRectFieldLayerFrame:
        case ReconRectFieldLayerBounds: return layerRect(object, field, value);
        case ReconRectFieldGuideFrame: return guideRect(object, value);
    }
}

ReconReadStatus ReconReadRect(id object, ReconRectField field, CGRect *value) {
    *value = CGRectZero;
    @try {
        return readRectField(object, field, value);
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

static ReconReadStatus readSizeField(id object, ReconSizeField field, CGSize *value) {
    switch (field) {
        case ReconSizeFieldViewIntrinsicContentSize:
            if (!ReconIsKindOf(object, ReconNativeClassView)) return ReconReadStatusUnavailable;
            *value = [(NSView *)object intrinsicContentSize];
            return ReconReadStatusValue;
    }
}

ReconReadStatus ReconReadSize(id object, ReconSizeField field, CGSize *value) {
    *value = CGSizeZero;
    @try {
        return readSizeField(object, field, value);
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

ReconReadStatus ReconReadViewRectInWindow(id view, CGRect *value) {
    *value = CGRectZero;
    @try {
        if (!ReconIsKindOf(view, ReconNativeClassView)) return ReconReadStatusUnavailable;
        NSView *target = view;
        if (!target.window) return ReconReadStatusUnavailable;
        *value = [target convertRect:target.bounds toView:nil];
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

ReconReadStatus ReconConvertWindowRectToScreen(id window, CGRect rect, CGRect *value) {
    *value = CGRectZero;
    @try {
        if (!ReconIsKindOf(window, ReconNativeClassWindow)) return ReconReadStatusUnavailable;
        *value = [(NSWindow *)window convertRectToScreen:rect];
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

/// AppKit's global screen space puts its origin at the bottom left of the screen that owns
/// (0,0) — `NSScreen.screens.firstObject` — while accessibility reports the same logical points
/// from that screen's top left. The flip is that screen's height and nothing else: both spaces
/// are in logical points, so no backing scale factor takes part in it.
double ReconPrimaryScreenHeight(void) {
    @try {
        NSArray<NSScreen *> *screens = NSScreen.screens;
        return screens.count ? NSHeight(screens.firstObject.frame) : NAN;
    } @catch (id exception) {
        return NAN;
    }
}

CGRect ReconScreenTopLeftRect(CGRect rect, double primaryScreenHeight) {
    return CGRectMake(rect.origin.x, primaryScreenHeight - CGRectGetMaxY(rect),
                      rect.size.width, rect.size.height);
}
