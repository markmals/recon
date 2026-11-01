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

ReconObjectKind ReconKindOf(id object) {
    if ([object isKindOfClass:NSWindow.class]) return ReconObjectKindWindow;
    if ([object isKindOfClass:NSView.class]) return ReconObjectKindView;
    if ([object isKindOfClass:CALayer.class]) return ReconObjectKindLayer;
    if ([object isKindOfClass:NSCell.class]) return ReconObjectKindCell;
    if ([object isKindOfClass:NSLayoutGuide.class]) return ReconObjectKindLayoutGuide;
    return ReconObjectKindUnsupported;
}

BOOL ReconIsKindOf(id object, ReconNativeClass nativeClass) {
    return [object isKindOfClass:classForNative(nativeClass)];
}
