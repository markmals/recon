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

/// Runs on the main thread and brings back a value that crosses threads on its own.
func onMainValue<Value: Sendable>(_ body: @MainActor () -> Value) -> Value {
    DispatchQueue.main.sync { MainActor.assumeIsolated(body) }
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
            identifier: identifier, helperBuild: "test-build", protocolVersion: 2)
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

    /// One read's budget and coverage, for the parts of the walk no action reaches directly.
    func work(_ deadline: Duration = .seconds(30)) throws -> Work {
        let request = try InspectionRequest(
            .tree, root: nil, windowId: nil, depth: 3, limit: 200, fields: nil, kind: nil,
            className: nil, identifier: nil)
        return Work(request: request, session: session, deadline: .now + deadline)
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

/// A view whose type check raises once armed, as an application's own override may. AppKit
/// itself asks views their kind while building them, so it raises only around the read under test.
final class RaisingKindView: NSView {
    nonisolated(unsafe) static var armed = false

    override func isKind(of aClass: AnyClass) -> Bool {
        if Self.armed {
            NSException(name: .genericException, reason: "unclassifiable", userInfo: nil).raise()
        }
        return super.isKind(of: aClass)
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

/// An attributed value that reports a run outside itself, as an application's own
/// `NSAttributedString` subclass may.
final class LyingRunString: NSAttributedString {
    override var string: String { "lying" }

    override func attributes(at location: Int, effectiveRange range: NSRangePointer?)
        -> [NSAttributedString.Key: Any]
    { [:] }

    override func attribute(
        _ attrName: NSAttributedString.Key, at location: Int,
        longestEffectiveRange range: NSRangePointer?, in rangeLimit: NSRange
    ) -> Any? {
        range?.pointee = NSRange(location: Int.max, length: 1)
        return NSFont.systemFont(ofSize: 12)
    }
}

/// A cell whose attributed value lies about where its runs are.
final class LyingRunCell: NSCell {
    override var attributedStringValue: NSAttributedString {
        get { LyingRunString() }
        set {}
    }
}

/// One serialized parent: every suite drives the same process-wide session and main thread.
@Suite(.serialized)
struct AppKitInspection {}

/// A layer whose superlayer is whatever the test says, including itself or a cycle.
final class CyclicLayer: CALayer {
    nonisolated(unsafe) var cycle: CALayer?

    override var superlayer: CALayer? { cycle }
}

/// A view that reports its own coordinate system as top-left, as a flipped view does.
final class FlippedView: NSView {
    override var isFlipped: Bool { true }
}
