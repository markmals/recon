import AppKit
import CoreGraphics
import Foundation

extension ReconNative {
    /// Every window the running application holds, or nothing where there is no application
    /// object at all.
    public static func applicationWindows() -> ReconRead<[AnyObject]> {
        switch application() {
        case .value(let application):
            switch Message.object(Fields.applicationWindows, to: application) {
            case .value(let windows): return Message.elements(of: windows)
            case .unavailable: return .value([])
            case .failed: return .failed
            }
        case .unavailable: return .unavailable
        case .failed: return .failed
        }
    }

    public static func applicationWindow(number: Int) -> AnyObject? {
        guard case .value(let application) = application() else { return nil }
        return Message.object(Fields.windowWithWindowNumber, to: application, at: number).value
    }

    /// Every window the WindowServer reports, whoever owns it. This is the system's own
    /// inventory rather than anything the inspected application answers.
    public static func windowServerInventory() -> [ReconWindowServerEntry] {
        let inventory =
            CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
        return inventory.compactMap { entry in
            guard let number = entry[kCGWindowNumber as String] as? Int else { return nil }
            return ReconWindowServerEntry(
                number: number, ownerPid: entry[kCGWindowOwnerPID as String] as? Int ?? 0,
                onscreen: entry[kCGWindowIsOnscreen as String] as? Bool ?? false)
        }
    }

    /// The height of the screen that owns the global origin, or NaN where none reports one.
    public static var primaryScreenHeight: Double {
        guard case .value(let screens) = Message.object(Fields.screens, to: NSScreen.self),
            case .value(let list) = Message.elements(of: screens), let primary = list.first,
            case .value(let frame) = Message.rect(Fields.frame, to: primary)
        else { return .nan }
        return frame.height
    }

    /// AppKit's global screen space puts its origin at the bottom left of the screen that owns
    /// (0,0), while accessibility reports the same logical points from that screen's top left.
    /// The flip is that screen's height and nothing else: both spaces are in logical points, so
    /// no backing scale factor takes part in it.
    public static func screenTopLeft(_ rect: CGRect, primaryScreenHeight: Double) -> CGRect {
        CGRect(
            x: rect.origin.x, y: primaryScreenHeight - rect.maxY, width: rect.width,
            height: rect.height)
    }

    /// `view.bounds` in its window's coordinates.
    public static func rectInWindow(of view: AnyObject) -> ReconRead<CGRect> {
        guard isKind(view, of: .view) else { return .unavailable }
        guard case .value = Message.object(Fields.window, to: view) else { return .unavailable }
        guard case .value(let bounds) = Message.rect(Fields.bounds, to: view) else {
            return .failed
        }
        return Message.rect(Fields.convertRectToView, to: view, of: bounds, in: nil)
    }

    public static func screenRect(of window: AnyObject, _ rect: CGRect) -> ReconRead<CGRect> {
        guard isKind(window, of: .window) else { return .unavailable }
        return Message.rect(Fields.convertRectToScreen, to: window, of: rect)
    }

    /// The application object, read from AppKit's own global rather than asked for: asking
    /// for it would create one in a process that has none. AppKit publishes the global on its
    /// main thread, which is the only thread these reads are valid on anyway.
    private static func application() -> ReconRead<AnyObject> {
        guard Thread.isMainThread else { return .unavailable }
        return MainActor.assumeIsolated { NSApp }.map { ReconRead.value($0) } ?? .unavailable
    }
}
