import Foundation
import Synchronization

/// One target session's access, owned by the runtime's host. `begin` must check that access is
/// still current and commit the invocation as started in one critical section that close also
/// takes, so a close can neither interrupt started work nor let revoked work begin.
public struct ReconAccess: Sendable {
    public let begin: @Sendable () -> Bool
    public let isRevoked: @Sendable () -> Bool
    /// Called once scheduled work can no longer start or has finished, even after a timed-out
    /// wait: the host releases its single-flight guard here, not when the caller gives up.
    public let settled: @Sendable () -> Void

    public init(
        begin: @escaping @Sendable () -> Bool, isRevoked: @escaping @Sendable () -> Bool,
        settled: @escaping @Sendable () -> Void = {}
    ) {
        self.begin = begin
        self.isRevoked = isRevoked
        self.settled = settled
    }

    /// Access that is always current: the shape a test host or a sessionless action needs.
    public static let unrestricted = ReconAccess(begin: { true }, isRevoked: { false })
}

/// Shared between the waiting caller and the scheduled work, which may outlive the caller's wait.
final class Execution: Sendable {
    private struct State {
        var started = false
        var answer: Result<Data, any Error>?
    }

    private let state = Mutex(State())

    var started: Bool { state.withLock { $0.started } }
    var answer: Result<Data, any Error>? { state.withLock { $0.answer } }

    /// Commits the invocation as started, under the lock the host's `begin` takes.
    func start(_ gate: () -> Bool) -> Bool {
        state.withLock { state in
            state.started = gate()
            return state.started
        }
    }

    func finish(_ body: () throws -> Data) {
        record(Result { try body() })
    }

    func record(_ answer: Result<Data, any Error>) {
        state.withLock { $0.answer = answer }
    }
}
