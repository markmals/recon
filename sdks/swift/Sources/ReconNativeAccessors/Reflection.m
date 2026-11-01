#import "ReconAccessorsInternal.h"

#import <objc/runtime.h>

static Class classForNative(ReconNativeClass nativeClass) {
    switch (nativeClass) {
        case ReconNativeClassWindow: return NSWindow.class;
        case ReconNativeClassView: return NSView.class;
        case ReconNativeClassLayer: return CALayer.class;
        case ReconNativeClassCell: return NSCell.class;
        case ReconNativeClassLayoutGuide: return NSLayoutGuide.class;
        case ReconNativeClassControl: return NSControl.class;
        case ReconNativeClassTextView: return NSTextView.class;
        case ReconNativeClassTextField: return NSTextField.class;
        case ReconNativeClassTextFieldCell: return NSTextFieldCell.class;
        case ReconNativeClassBox: return NSBox.class;
        case ReconNativeClassVisualEffectView: return NSVisualEffectView.class;
    }
}

NSString *ReconClassName(id object) {
    return NSStringFromClass(object_getClass(object));
}

NSArray<NSString *> *ReconSuperclassNames(id object, NSUInteger limit) {
    NSMutableArray<NSString *> *names = [NSMutableArray new];
    Class cls = class_getSuperclass(object_getClass(object));
    while (cls && names.count < limit) {
        [names addObject:NSStringFromClass(cls)];
        cls = class_getSuperclass(cls);
    }
    return names;
}

// Classification walks the runtime's class chain instead of sending -isKindOfClass:, which an
// application may override and which may raise. No message reaches the target, so no Objective-C
// exception can start here, and every Swift caller can classify an object outside an accessor.
static BOOL inherits(id object, Class ancestor) {
    for (Class cls = object_getClass(object); cls; cls = class_getSuperclass(cls)) {
        if (cls == ancestor) return YES;
    }
    return NO;
}

ReconObjectKind ReconKindOf(id object) {
    if (inherits(object, NSWindow.class)) return ReconObjectKindWindow;
    if (inherits(object, NSView.class)) return ReconObjectKindView;
    if (inherits(object, CALayer.class)) return ReconObjectKindLayer;
    if (inherits(object, NSCell.class)) return ReconObjectKindCell;
    if (inherits(object, NSLayoutGuide.class)) return ReconObjectKindLayoutGuide;
    return ReconObjectKindUnsupported;
}

BOOL ReconIsKindOf(id object, ReconNativeClass nativeClass) {
    return inherits(object, classForNative(nativeClass));
}
