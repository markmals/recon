import Foundation
import ReconCatalystAccessors
import ReconExtension

/// A message the target refused: it raised, or answered something of a kind Recon will not use.
///
/// A target may implement any of UIKit's methods itself, so this is the target answering rather
/// than UIKit lacking the method, and it is reported as a failure rather than as an absent
/// capability.
struct TargetRefusedMessage: Error, CustomStringConvertible {
    let what: String

    var description: String { "the target refused \(what)" }
}

/// The UIKit of a Mac Catalyst target, as the one interaction operation reaches it.
///
/// Naming the surface is what makes the resolution testable at all: no `UIWindow` can exist in a
/// test process, and a Catalyst target is the only place `TargetUIKit` ever runs. Everything a
/// test can get wrong — which window a WindowServer number resolves to, what a missing or
/// ambiguous match does, what is reported on either side of the change — lives above this
/// protocol; below it is one message per fact and nothing else.
protocol CatalystUIKit {
    /// Every `UIWindow` of every connected `UIWindowScene`.
    func windows() throws -> [AnyObject]
    /// The WindowServer window a `UIWindow` is hosted in, or nil where it is hosted in none.
    func hostWindowNumber(of window: AnyObject) throws -> Int?
    /// UIKit's own key window, or nil where UIKit reports none.
    func keyWindow() throws -> AnyObject?
    /// `UIWindow.makeKey()`, the only change this extension makes to a target.
    func makeKey(_ window: AnyObject) throws
}

/// The live UIKit of the process the helper is resident in, reached through the Objective-C
/// runtime because the helper links no UIKit.
struct TargetUIKit: CatalystUIKit {
    /// The most members a UIKit collection may report. No Catalyst application comes near it;
    /// the bound is what stops a target that claims a length no process could hold from
    /// deciding an allocation.
    static let collectionLimit = 4096

    func windows() throws -> [AnyObject] {
        let application = try sharedApplication()
        var scenes: NSArray?
        switch ReconUIKitConnectedScenes(application, UInt(Self.collectionLimit), &scenes) {
        case .value:
            break
        case .unavailable:
            throw Self.unavailable("the target's UIApplication reports no connected scenes")
        default:
            throw TargetRefusedMessage(what: "a read of its connected scenes")
        }
        var windows: [AnyObject] = []
        for scene in (scenes as? [AnyObject]) ?? [] {
            var sceneWindows: NSArray?
            switch ReconUIKitSceneWindows(scene, UInt(Self.collectionLimit), &sceneWindows) {
            case .value:
                // A scene with no windows contributes none; that is not a failure.
                windows.append(contentsOf: (sceneWindows as? [AnyObject]) ?? [])
            case .unavailable:
                throw Self.unavailable("a connected UIWindowScene reports no windows")
            default:
                throw TargetRefusedMessage(what: "a read of a UIWindowScene's windows")
            }
        }
        return windows
    }

    func hostWindowNumber(of window: AnyObject) throws -> Int? {
        var number = 0
        switch ReconUIKitHostWindowNumber(window, &number) {
        case .value:
            return number
        // A UIWindow attached to no host window is hosted in no WindowServer window, which is a
        // fact about that window rather than an unreadable one.
        case .unavailable:
            return nil
        default:
            throw TargetRefusedMessage(what: "a read of a UIWindow's host window")
        }
    }

    func keyWindow() throws -> AnyObject? {
        let application = try sharedApplication()
        var window: AnyObject?
        switch ReconUIKitKeyWindow(application, &window) {
        case .value:
            return window
        case .unavailable:
            throw Self.unavailable("the target's UIApplication cannot report its key window")
        default:
            throw TargetRefusedMessage(what: "a read of its UIKit key window")
        }
    }

    func makeKey(_ window: AnyObject) throws {
        switch ReconUIKitMakeKeyWindow(window) {
        case .value:
            return
        case .unavailable:
            throw Self.unavailable("the resolved UIWindow does not implement makeKey()")
        default:
            throw TargetRefusedMessage(what: "makeKey() on the resolved UIWindow")
        }
    }

    private func sharedApplication() throws -> AnyObject {
        var application: AnyObject?
        let status = ReconUIKitSharedApplication(&application)
        if status == .failed {
            throw TargetRefusedMessage(what: "a read of its UIApplication")
        }
        guard status == .value, let application else {
            throw Self.unavailable("the target has no UIKit application; it is not Mac Catalyst")
        }
        return application
    }

    /// UIKit answered nothing. Recon never substitutes another mechanism for the one an
    /// operation names, so an unreadable prerequisite ends the operation before any change.
    private static func unavailable(_ what: String) -> ReconActionError {
        .unavailable("Catalyst interaction is unavailable: \(what)")
    }
}
