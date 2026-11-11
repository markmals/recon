#import <Foundation/Foundation.h>

NS_ASSUME_NONNULL_BEGIN

/// The whole Objective-C boundary between Recon's helper and the UIKit of a Mac Catalyst target.
///
/// The helper is a macOS library: it links AppKit and never links UIKit, so every class and
/// selector named here is resolved in the target at run time. A target that is not a Catalyst
/// application resolves none of them and every function answers `Unavailable`, which is how an
/// AppKit target refuses this operation rather than being messaged as if it were Catalyst.
///
/// Each function answers one fact, sends only the messages that fact needs, and sends them
/// inside one `@try`: a target may implement any of these methods itself, so no Objective-C
/// exception can reach the Swift caller. Nothing here takes a selector, a class name, or a key
/// path from its caller, and a client can never supply one: the closed set of messages Recon's
/// interaction operation sends is the set of functions in this file.
typedef NS_ENUM(NSInteger, ReconUIKitStatus) {
    ReconUIKitStatusValue,
    /// UIKit is not loaded, or the object does not implement the method.
    ReconUIKitStatusUnavailable,
    /// The message raised, or answered something of a kind Recon will not use.
    ReconUIKitStatusFailed,
};

/// `+[UIApplication sharedApplication]`. Unavailable where UIKit is absent or has no instance.
ReconUIKitStatus ReconUIKitSharedApplication(id _Nullable *_Nonnull result);

/// The `UIWindowScene`s of `-[UIApplication connectedScenes]`, snapshotted inside the boundary.
///
/// A target that reports more scenes than `limit` is refused rather than copied: the target
/// names the number, so the target must not decide an allocation.
ReconUIKitStatus ReconUIKitConnectedScenes(id application, NSUInteger limit,
                                           NSArray *_Nullable *_Nonnull result);

/// The `UIWindow`s of `-[UIWindowScene windows]`, snapshotted inside the boundary and bounded
/// by `limit` the same way.
ReconUIKitStatus ReconUIKitSceneWindows(id scene, NSUInteger limit,
                                        NSArray *_Nullable *_Nonnull result);

/// `-[UIApplication keyWindow]`: UIKit's own account of which window is key.
///
/// A `Value` result with a `nil` window is UIKit reporting that there is none, which is a fact
/// about the target; `Unavailable` is UIKit being unable to answer at all.
ReconUIKitStatus ReconUIKitKeyWindow(id application, id _Nullable *_Nonnull result);

/// The WindowServer window a Catalyst `UIWindow` is hosted in:
/// `-[UIWindow nsWindowProxy]`, `-[UINSWindowProxy attachedWindow]`, `-[NSWindow windowNumber]`.
///
/// Unavailable where the window is attached to no host window, which a `UIWindow` that is not on
/// screen is.
ReconUIKitStatus ReconUIKitHostWindowNumber(id window, NSInteger *result);

/// `-[UIWindow makeKeyWindow]`: the one message this boundary sends that changes the target.
ReconUIKitStatus ReconUIKitMakeKeyWindow(id window);

NS_ASSUME_NONNULL_END
