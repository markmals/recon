#import "ReconNativeSupport.h"
#import <AppKit/AppKit.h>
#import <objc/message.h>
#import <objc/runtime.h>

#if !__has_feature(objc_arc)
#error "ReconNativeSupport requires ARC"
#endif

static SEL selectorFor(ReconStringField field) {
    switch (field) {
        case ReconStringFieldTitle: return @selector(title);
        case ReconStringFieldToolTip: return @selector(toolTip);
        case ReconStringFieldIdentifier: return @selector(identifier);
    }
    return NULL;
}

ReconReadStatus ReconReadString(id object, ReconStringField field, NSString **value) {
    *value = nil;
    SEL selector = selectorFor(field);
    if (!selector || ![object respondsToSelector:selector]) return ReconReadStatusUnavailable;
    @try {
        id result = ((id (*)(id, SEL))objc_msgSend)(object, selector);
        if (!result) return ReconReadStatusUnavailable;
        if (![result isKindOfClass:NSString.class]) return ReconReadStatusFailed;
        *value = result;
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

ReconReadStatus ReconReadApplicationWindows(NSArray **windows) {
    *windows = nil;
    @try {
        *windows = [NSApp.windows copy];
        return *windows ? ReconReadStatusValue : ReconReadStatusUnavailable;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

ReconReadStatus ReconReadContentView(id window, id *view) {
    *view = nil;
    if (![window isKindOfClass:NSWindow.class]) return ReconReadStatusUnavailable;
    @try {
        *view = [(NSWindow *)window contentView];
        return *view ? ReconReadStatusValue : ReconReadStatusUnavailable;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

ReconReadStatus ReconReadSubviews(id view, NSArray **subviews) {
    *subviews = nil;
    if (![view isKindOfClass:NSView.class]) return ReconReadStatusUnavailable;
    @try {
        *subviews = [[(NSView *)view subviews] copy];
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

ReconReadStatus ReconReadWindowNumber(id window, NSInteger *number) {
    *number = 0;
    if (![window isKindOfClass:NSWindow.class]) return ReconReadStatusUnavailable;
    @try {
        *number = [(NSWindow *)window windowNumber];
        return ReconReadStatusValue;
    } @catch (id exception) {
        return ReconReadStatusFailed;
    }
}

NSString *ReconClassName(id object) {
    return NSStringFromClass(object_getClass(object));
}
