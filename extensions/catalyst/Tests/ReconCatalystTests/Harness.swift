import Foundation
import ReconNativeSupport
import Testing

@testable import ReconCatalyst

/// A stand-in for one live `UIWindow`. No UIKit object can exist in a test process, so the
/// resolution is exercised against objects that only have to be distinct and referenceable —
/// which is all the resolution itself ever asks of a window.
final class StubWindow: NSObject {}

/// A UIKit a test states: which windows exist, which WindowServer window hosts each, what UIKit
/// calls key, and what `makeKey()` does to that.
final class StubUIKit: CatalystUIKit {
    var windowList: [StubWindow] = []
    /// A window absent from this table is hosted in no WindowServer window.
    var hosts: [ObjectIdentifier: Int] = [:]
    var key: StubWindow?
    /// What UIKit reports as key once `makeKey()` has been sent. Reporting the window that was
    /// made key is UIKit honouring the change; reporting anything else is it not doing so.
    var keyAfterMakeKey: (StubWindow) -> StubWindow? = { $0 }

    var windowsError: (any Error)?
    var hostError: (any Error)?
    var keyWindowError: (any Error)?
    var makeKeyError: (any Error)?

    /// Every window `makeKey()` was sent to, in order. A refusal must leave this empty.
    private(set) var madeKey: [StubWindow] = []

    init() {}

    /// Adds a window hosted in `windowId`, or in no WindowServer window when it is nil.
    @discardableResult
    func window(hostedIn windowId: Int?) -> StubWindow {
        let window = StubWindow()
        windowList.append(window)
        if let windowId { hosts[ObjectIdentifier(window)] = windowId }
        return window
    }

    func windows() throws -> [AnyObject] {
        if let windowsError { throw windowsError }
        return windowList
    }

    func hostWindowNumber(of window: AnyObject) throws -> Int? {
        if let hostError { throw hostError }
        return hosts[ObjectIdentifier(window)]
    }

    func keyWindow() throws -> AnyObject? {
        if let keyWindowError { throw keyWindowError }
        return key
    }

    func makeKey(_ window: AnyObject) throws {
        if let makeKeyError { throw makeKeyError }
        guard let window = window as? StubWindow else {
            throw TargetRefusedMessage(what: "makeKey() on an object that is not a window")
        }
        madeKey.append(window)
        key = keyAfterMakeKey(window)
    }
}

/// A session, its handle registry, and the operation as the helper assembles it.
struct Interactor {
    let uiKit: StubUIKit
    let session: ReconNativeSession

    init(_ identifier: String = "catalyst-test-session") {
        if let open = ReconNativeSession.current {
            ReconNativeSession.close(identifier: open.identifier)
        }
        ReconNativeSession.open(
            identifier: identifier, helperBuild: "test-build", protocolVersion: 2)
        session = ReconNativeSession.current!
        uiKit = StubUIKit()
    }

    var operation: MakeWindowKey { MakeWindowKey(uiKit: uiKit, session: session) }

    func makeKey(window windowId: Int, cancelled: Bool = false) throws -> KeyWindowChange {
        try operation.perform(windowId: windowId, isCancelled: { cancelled })
    }

    func handle(for window: StubWindow) -> String { session.handles.handle(for: window)! }
}

/// One serialized parent: every suite drives the same process-wide session, and opening one
/// revokes the handles of the session before it.
@Suite(.serialized)
struct CatalystInteraction {}
