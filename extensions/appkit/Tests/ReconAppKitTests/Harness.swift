import AppKit
import Foundation
import ReconExtensionTesting
import ReconNativeSupport
import Testing

@testable import ReconAppKit

/// Builds AppKit objects on the thread AppKit requires, for a test that must not hold the main
/// thread while an action runs on it. An object crosses back as a reference the test only hands
/// to the registry; every read of it happens on the main thread, inside the action.
func onMain<Value: AnyObject>(_ body: @MainActor () -> Value) -> Value {
    DispatchQueue.main.sync { MainActor.assumeIsolated { Carried(body()) } }.value
}

func onMain(_ body: @MainActor () -> Void) {
    DispatchQueue.main.sync { MainActor.assumeIsolated(body) }
}

private final class Carried<Value: AnyObject>: @unchecked Sendable {
    let value: Value
    init(_ value: Value) { self.value = value }
}

/// A session, a handle registry, and a host, as the helper assembles them.
struct Inspector {
    let host: ReconTestHost
    let session: ReconNativeSession

    init(_ identifier: String = "test-session") throws {
        if let open = ReconNativeSession.current {
            ReconNativeSession.close(identifier: open.identifier)
        }
        ReconNativeSession.open(
            identifier: identifier, helperBuild: "test-build", protocolVersion: 1)
        session = ReconNativeSession.current!
        host = try ReconTestHost(AppKitInspector.reconExtension)
    }

    func handle(for object: AnyObject) -> String { session.handles.handle(for: object)! }

    func invoke(_ action: String, _ payload: [String: Any] = [:]) throws -> ReconTestOutcome {
        let data = try JSONSerialization.data(withJSONObject: payload)
        return try host.invoke(action, String(decoding: data, as: UTF8.self))
    }

    /// The decoded result, or a recorded failure.
    func read<Response: Decodable>(
        _ action: String, _ payload: [String: Any] = [:], as response: Response.Type
    ) throws -> Response {
        try invoke(action, payload).decode(response)
    }

    func failure(_ action: String, _ payload: [String: Any] = [:]) throws -> (
        code: String, stage: String, message: String
    )? {
        try invoke(action, payload).failure
    }
}

/// A view whose subviews list repeats one child, which a traversal must report once and then
/// reference.
final class RepeatingView: NSView {
    override var subviews: [NSView] {
        get {
            let children = super.subviews
            return children.isEmpty ? children : [children[0], children[0]]
        }
        set { super.subviews = newValue }
    }
}

/// A view whose identifier getter raises, as an application's own override may.
final class RaisingView: NSView {
    override var identifier: NSUserInterfaceItemIdentifier? {
        get {
            NSException(name: .genericException, reason: "unreadable", userInfo: nil).raise()
            return nil
        }
        set {}
    }
}

/// A cell whose attributed value alternates fonts every character, so a read of it is bounded
/// by how many runs it will examine rather than by the string's length.
final class AlternatingFontCell: NSCell {
    override var attributedStringValue: NSAttributedString {
        get {
            let value = NSMutableAttributedString(
                string: String(repeating: "a", count: 300))
            let font = NSFont.systemFont(ofSize: 12)
            for index in stride(from: 1, to: 300, by: 2) {
                value.addAttribute(.font, value: font, range: NSRange(location: index, length: 1))
            }
            return value
        }
        set {}
    }
}

/// One serialized parent: every suite drives the same process-wide session and main thread.
@Suite(.serialized)
struct AppKitInspection {}
