import Foundation
import ReconExtension
import ReconNativeSupport

/// One UIKit window: the handle this session names the live object by, and the WindowServer
/// window it is hosted in.
@ReconModel
struct UIKitWindow: Codable, Equatable, Sendable {
    /// The session-scoped handle for the live `UIWindow`.
    let handle: String
    /// The WindowServer window hosting it, or null where UIKit hosts it in none.
    let windowId: ReconNullable<Int>
}

/// What one make-window-key did: the `UIWindow` the selected WindowServer window resolved to,
/// and UIKit's own key window before and after the change.
///
/// Both sides of the change are reported because the caller decides what the change means: it
/// requires the new key window to be the one it selected, and it reports a key-window change it
/// made rather than reversing it.
@ReconModel
struct KeyWindowChange: Codable, Equatable, Sendable {
    let window: UIKitWindow
    let before: ReconNullable<UIKitWindow>
    let after: ReconNullable<UIKitWindow>
}

/// Making one window key inside a Mac Catalyst target.
///
/// The selected window is a WindowServer window number, which is the identity every Recon
/// command names a window by. Exactly one `UIWindow` must be hosted in it: a chord is addressed
/// to that window, and a window this resolution guessed at would send keystrokes somewhere
/// nobody asked for. A Catalyst `UIWindow` reaches its host window through the AppKit window its
/// scene is attached to, which is the same WindowServer window `ui windows` reports.
struct MakeWindowKey {
    let uiKit: any CatalystUIKit
    let session: ReconNativeSession

    /// `isCancelled` is the invocation's own deadline and revocation, which the caller holds.
    /// It is checked once, immediately before the change: nothing before it has touched the
    /// target, and nothing after it can be undone.
    func perform(windowId: Int, isCancelled: () -> Bool) throws -> KeyWindowChange {
        guard windowId > 0 else {
            throw ReconActionError.invalidArgument(
                "A WindowServer window id is a positive integer")
        }
        let selected = try resolve(windowId)
        let window = try identity(of: selected)
        let current = try uiKit.keyWindow()
        let before = try current.map { try identity(of: $0) }
        if isCancelled() { throw ReconActionError.cancelled }
        if let current, current === selected {
            return KeyWindowChange(
                window: window, before: ReconNullable(before), after: ReconNullable(before))
        }
        try uiKit.makeKey(selected)
        return KeyWindowChange(
            window: window, before: ReconNullable(before),
            after: ReconNullable(try keyWindowIdentity()))
    }

    /// The one `UIWindow` the selected WindowServer window hosts. None and several are both
    /// failures, and they are different ones: nothing hosts a window that is not there, while
    /// several leave undecided which window a chord would reach.
    private func resolve(_ windowId: Int) throws -> AnyObject {
        var matches: [AnyObject] = []
        for window in try uiKit.windows() {
            if try uiKit.hostWindowNumber(of: window) == windowId { matches.append(window) }
        }
        guard matches.count == 1 else {
            guard !matches.isEmpty else {
                throw ReconActionError.unavailable(
                    "No UIWindow of this target is hosted in WindowServer window \(windowId)")
            }
            throw ReconActionError.unavailable(
                "\(matches.count) UIWindows of this target are hosted in WindowServer window "
                    + "\(windowId); which one a chord would reach is undecided")
        }
        return matches[0]
    }

    private func keyWindowIdentity() throws -> UIKitWindow? {
        guard let key = try uiKit.keyWindow() else { return nil }
        return try identity(of: key)
    }

    private func identity(of window: AnyObject) throws -> UIKitWindow {
        guard let handle = session.handles.handle(for: window) else { throw Self.noHandle }
        return UIKitWindow(
            handle: handle, windowId: ReconNullable(try uiKit.hostWindowNumber(of: window)))
    }

    private static let noHandle = ReconActionError.unavailable(
        "Native handle capacity exhausted", stage: "registry")
}
