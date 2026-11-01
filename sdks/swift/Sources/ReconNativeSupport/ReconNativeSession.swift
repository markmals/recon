import Foundation

/// The target-resident access session an inspecting action runs inside.
///
/// The generic `ReconContext` exposes only a deadline and cancellation, and nothing else should
/// be added to it: an action that computes a greeting must not inherit a registry of live
/// framework objects. A session's handles and its published envelope are therefore native
/// support's, reached through `ReconNativeSession.current` by the actions that actually inspect
/// a target. The helper's runtime opens and closes it alongside the access it describes.
public final class ReconNativeSession: Sendable {
    public let identifier: String
    public let helperBuild: String
    public let protocolVersion: Int
    public let handles: ReconHandleRegistry

    private static let active = ActiveSession()

    public init(identifier: String, helperBuild: String, protocolVersion: Int) {
        self.identifier = identifier
        self.helperBuild = helperBuild
        self.protocolVersion = protocolVersion
        handles = ReconHandleRegistry()
    }

    /// The open session, or nil where no access has been granted.
    public static var current: ReconNativeSession? { active.session }

    /// Installs a session. Re-opening the same identifier keeps its handles, as re-opening
    /// access does; a different identifier revokes the one it replaces.
    public static func open(identifier: String, helperBuild: String, protocolVersion: Int) {
        active.open(
            ReconNativeSession(
                identifier: identifier, helperBuild: helperBuild, protocolVersion: protocolVersion))
    }

    /// Revokes the named session's handles. A read already running keeps the objects it holds.
    public static func close(identifier: String) {
        active.close(identifier)
    }
}

private final class ActiveSession: @unchecked Sendable {
    private let lock = NSLock()
    private var current: ReconNativeSession?

    var session: ReconNativeSession? {
        lock.lock()
        defer { lock.unlock() }
        return current
    }

    func open(_ session: ReconNativeSession) {
        lock.lock()
        defer { lock.unlock() }
        guard current?.identifier != session.identifier else { return }
        current?.handles.revoke()
        current = session
    }

    func close(_ identifier: String) {
        lock.lock()
        defer { lock.unlock() }
        guard current?.identifier == identifier else { return }
        current?.handles.revoke()
        current = nil
    }
}
