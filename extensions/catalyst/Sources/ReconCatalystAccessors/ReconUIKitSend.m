#import "ReconUIKitSend.h"

#import <objc/message.h>
#import <objc/runtime.h>

#if !__has_feature(objc_arc)
#error "ReconCatalystAccessors requires ARC"
#endif

/// What an object is, from the class chain the runtime already holds. Asking the object with
/// `-isKindOfClass:` would be one more message it could answer with an exception.
static BOOL reconInherits(id object, Class ancestor) {
    if (!object || !ancestor) return NO;
    for (Class candidate = object_getClass(object); candidate;
         candidate = class_getSuperclass(candidate)) {
        if (candidate == ancestor) return YES;
    }
    return NO;
}

/// The members of a target's collection that are of one class, copied here so that reading them
/// later messages this process's array rather than the target's live collection.
///
/// A collection that reports more members than `limit` is refused rather than reserved for: the
/// target names the number, so the target must not decide an allocation.
static ReconUIKitStatus reconSnapshot(id<NSFastEnumeration> collection, NSUInteger count,
                                      Class kept, NSUInteger limit, NSArray **result) {
    if (count > limit) return ReconUIKitStatusFailed;
    NSMutableArray *members = [NSMutableArray arrayWithCapacity:count];
    for (id member in collection) {
        if (reconInherits(member, kept)) [members addObject:member];
    }
    *result = [members copy];
    return ReconUIKitStatusValue;
}

ReconUIKitStatus ReconUIKitSharedApplication(id *result) {
    *result = nil;
    Class application = objc_getClass("UIApplication");
    if (!application) return ReconUIKitStatusUnavailable;
    SEL shared = NSSelectorFromString(@"sharedApplication");
    @try {
        if (![application respondsToSelector:shared]) return ReconUIKitStatusUnavailable;
        id instance = ((id (*)(Class, SEL))objc_msgSend)(application, shared);
        if (!instance) return ReconUIKitStatusUnavailable;
        *result = instance;
        return ReconUIKitStatusValue;
    } @catch (id exception) {
        return ReconUIKitStatusFailed;
    }
}

ReconUIKitStatus ReconUIKitConnectedScenes(id application, NSUInteger limit, NSArray **result) {
    *result = nil;
    Class windowScene = objc_getClass("UIWindowScene");
    if (!windowScene) return ReconUIKitStatusUnavailable;
    SEL connected = NSSelectorFromString(@"connectedScenes");
    @try {
        if (![application respondsToSelector:connected]) return ReconUIKitStatusUnavailable;
        id scenes = ((id (*)(id, SEL))objc_msgSend)(application, connected);
        if (!scenes) return ReconUIKitStatusUnavailable;
        if (!reconInherits(scenes, NSSet.class)) return ReconUIKitStatusFailed;
        NSSet *connectedScenes = scenes;
        return reconSnapshot(connectedScenes, connectedScenes.count, windowScene, limit, result);
    } @catch (id exception) {
        return ReconUIKitStatusFailed;
    }
}

ReconUIKitStatus ReconUIKitSceneWindows(id scene, NSUInteger limit, NSArray **result) {
    *result = nil;
    Class window = objc_getClass("UIWindow");
    if (!window) return ReconUIKitStatusUnavailable;
    SEL windows = NSSelectorFromString(@"windows");
    @try {
        if (![scene respondsToSelector:windows]) return ReconUIKitStatusUnavailable;
        id list = ((id (*)(id, SEL))objc_msgSend)(scene, windows);
        if (!list) return ReconUIKitStatusUnavailable;
        if (!reconInherits(list, NSArray.class)) return ReconUIKitStatusFailed;
        NSArray *sceneWindows = list;
        return reconSnapshot(sceneWindows, sceneWindows.count, window, limit, result);
    } @catch (id exception) {
        return ReconUIKitStatusFailed;
    }
}

ReconUIKitStatus ReconUIKitKeyWindow(id application, id *result) {
    *result = nil;
    Class window = objc_getClass("UIWindow");
    if (!window) return ReconUIKitStatusUnavailable;
    SEL keyWindow = NSSelectorFromString(@"keyWindow");
    @try {
        if (![application respondsToSelector:keyWindow]) return ReconUIKitStatusUnavailable;
        id key = ((id (*)(id, SEL))objc_msgSend)(application, keyWindow);
        // No key window is an answer, not a missing one.
        if (!key) return ReconUIKitStatusValue;
        if (!reconInherits(key, window)) return ReconUIKitStatusFailed;
        *result = key;
        return ReconUIKitStatusValue;
    } @catch (id exception) {
        return ReconUIKitStatusFailed;
    }
}

ReconUIKitStatus ReconUIKitHostWindowNumber(id window, NSInteger *result) {
    *result = 0;
    Class hostClass = objc_getClass("NSWindow");
    if (!hostClass) return ReconUIKitStatusUnavailable;
    SEL proxy = NSSelectorFromString(@"nsWindowProxy");
    SEL attached = NSSelectorFromString(@"attachedWindow");
    SEL number = NSSelectorFromString(@"windowNumber");
    @try {
        if (![window respondsToSelector:proxy]) return ReconUIKitStatusUnavailable;
        id bridge = ((id (*)(id, SEL))objc_msgSend)(window, proxy);
        if (!bridge) return ReconUIKitStatusUnavailable;
        if (![bridge respondsToSelector:attached]) return ReconUIKitStatusUnavailable;
        id host = ((id (*)(id, SEL))objc_msgSend)(bridge, attached);
        if (!host) return ReconUIKitStatusUnavailable;
        if (!reconInherits(host, hostClass)) return ReconUIKitStatusFailed;
        if (![host respondsToSelector:number]) return ReconUIKitStatusUnavailable;
        *result = ((NSInteger (*)(id, SEL))objc_msgSend)(host, number);
        return ReconUIKitStatusValue;
    } @catch (id exception) {
        return ReconUIKitStatusFailed;
    }
}

ReconUIKitStatus ReconUIKitMakeKeyWindow(id window) {
    Class windowClass = objc_getClass("UIWindow");
    if (!windowClass) return ReconUIKitStatusUnavailable;
    // The one changing message is sent only to an object the runtime says is a UIWindow.
    if (!reconInherits(window, windowClass)) return ReconUIKitStatusFailed;
    SEL makeKey = NSSelectorFromString(@"makeKeyWindow");
    @try {
        if (![window respondsToSelector:makeKey]) return ReconUIKitStatusUnavailable;
        ((void (*)(id, SEL))objc_msgSend)(window, makeKey);
        return ReconUIKitStatusValue;
    } @catch (id exception) {
        return ReconUIKitStatusFailed;
    }
}
