import Foundation
import ReconExtension
import Synchronization

public struct ReconFailure: Error, Sendable, Equatable {
    public let code: String
    public let stage: String
    public let message: String
    public let targetExecutionMayContinue: Bool?

    init(_ code: String, _ stage: String, _ message: String, mayContinue: Bool? = nil) {
        self.code = code
        self.stage = stage
        self.message = message
        self.targetExecutionMayContinue = mayContinue
    }

    public init(code: String, stage: String, message: String) {
        self.init(code, stage, message)
    }
}

public enum ReconOutcome: Sendable, Equatable {
    case success(Data)
    case failure(ReconFailure)
}

public enum ReconAssemblyError: Error, Equatable {
    case duplicateExtension(String)
    case duplicateAction(extension: String, action: String)
}

/// One target session's access, owned by native support. `begin` must check that access is still
/// current and commit the invocation as started in one critical section that close also takes.
public struct ReconAccess: Sendable {
    public let begin: @Sendable () -> Bool
    public let isRevoked: @Sendable () -> Bool
    /// Called once scheduled work can no longer start or has finished, even after a timed-out wait.
    public let settled: @Sendable () -> Void

    public init(
        begin: @escaping @Sendable () -> Bool, isRevoked: @escaping @Sendable () -> Bool,
        settled: @escaping @Sendable () -> Void = {}
    ) {
        self.begin = begin
        self.isRevoked = isRevoked
        self.settled = settled
    }
}

/// The assembled, closed set of actions and the scheduling that preserves their isolation.
public final class ReconRuntime: Sendable {
    public static let maximumResultBytes = 8 * 1024 * 1024
    private let actions: [String: [String: ReconActionDescriptor]]

    public init(assembling extensions: [ReconExtensionDescriptor]) throws(ReconAssemblyError) {
        var actions: [String: [String: ReconActionDescriptor]] = [:]
        for descriptor in extensions {
            guard actions[descriptor.identifier] == nil else {
                throw .duplicateExtension(descriptor.identifier)
            }
            var named: [String: ReconActionDescriptor] = [:]
            for action in descriptor.actions {
                guard named[action.name] == nil else {
                    throw .duplicateAction(extension: descriptor.identifier, action: action.name)
                }
                named[action.name] = action
            }
            actions[descriptor.identifier] = named
        }
        self.actions = actions
    }

    public func action(_ extensionID: String, _ name: String) -> ReconActionDescriptor? {
        actions[extensionID]?[name]
    }

    /// Discovery metadata only: listing it neither touches a target nor runs extension code.
    public func discovery() -> String {
        let entries = actions.keys.sorted().flatMap { extensionID in
            actions[extensionID]!.keys.sorted().map { name in
                let action = actions[extensionID]![name]!
                return "{\"extension\":\"\(extensionID)\",\"action\":\"\(name)\","
                    + "\"input\":\(action.input.jsonSchema),\"output\":\(action.output.jsonSchema)}"
            }
        }
        return "[\(entries.joined(separator: ","))]"
    }

    /// Validates, schedules, and awaits one invocation. Blocks the caller; never call it on the
    /// main thread, which main-actor actions need.
    public func invoke(
        _ extensionID: String, _ name: String, payload: Data, access: ReconAccess,
        deadline: ContinuousClock.Instant
    ) -> ReconOutcome {
        guard let action = action(extensionID, name) else {
            access.settled()
            return .failure(ReconFailure("INVALID_ARGUMENT", "operation", "Unsupported closed operation"))
        }
        if let violation = SchemaValidator.violation(in: payload, against: action.input) {
            access.settled()
            return .failure(ReconFailure("INVALID_ARGUMENT", "input", violation))
        }
        let execution = Execution()
        let done = DispatchSemaphore(value: 0)
        schedule(action.invocation, payload, access, deadline, execution) {
            access.settled()
            done.signal()
        }

        let remaining = max(ContinuousClock.now.duration(to: deadline), .zero)
        let nanoseconds = remaining.components.seconds * 1_000_000_000
            + remaining.components.attoseconds / 1_000_000_000
        if done.wait(timeout: .now() + .nanoseconds(Int(nanoseconds))) == .timedOut {
            let started = execution.started
            return .failure(
                ReconFailure(
                    "INSPECTION_TIMEOUT", started ? "target-execution" : "queue",
                    "Action exceeded its deadline", mayContinue: started))
        }
        guard let answer = execution.answer else {
            return access.isRevoked()
                ? .failure(ReconFailure("STALE_SESSION", "queue", "Access was revoked before the action started"))
                : .failure(
                    ReconFailure("INSPECTION_TIMEOUT", "queue", "Queued action expired", mayContinue: false))
        }
        return outcome(answer, output: action.output)
    }

    private func schedule(
        _ invocation: ReconInvocation, _ payload: Data, _ access: ReconAccess,
        _ deadline: ContinuousClock.Instant, _ execution: Execution,
        _ finished: @escaping @Sendable () -> Void
    ) {
        let context = ReconContext(deadline: deadline, revoked: access.isRevoked)
        let gate: @Sendable () -> Bool = { ContinuousClock.now < deadline && access.begin() }
        switch invocation {
        case .mainActor(let adapter):
            DispatchQueue.main.async {
                if execution.start(gate) {
                    execution.finish { try MainActor.assumeIsolated { try adapter(payload, context) } }
                }
                finished()
            }
        case .nonisolated(let adapter):
            DispatchQueue.global().async {
                if execution.start(gate) { execution.finish { try adapter(payload, context) } }
                finished()
            }
        case .mainActorAsync(let adapter):
            Task { @MainActor in
                if execution.start(gate) {
                    do { execution.record(.success(try await adapter(payload, context))) } catch {
                        execution.record(.failure(error))
                    }
                }
                finished()
            }
        case .nonisolatedAsync(let adapter):
            Task.detached {
                if execution.start(gate) {
                    do { execution.record(.success(try await adapter(payload, context))) } catch {
                        execution.record(.failure(error))
                    }
                }
                finished()
            }
        }
    }

    private func outcome(_ answer: Result<Data, any Error>, output: ReconSchemaNode) -> ReconOutcome {
        switch answer {
        case .success(let data):
            if data.count > Self.maximumResultBytes {
                return .failure(ReconFailure("INSPECTION_UNAVAILABLE", "response-size", "Result exceeds 8 MiB"))
            }
            if let violation = SchemaValidator.violation(in: data, against: output) {
                return .failure(
                    ReconFailure(
                        "INSPECTION_FAILED", "encoding",
                        "Result does not match its published schema: \(violation)"))
            }
            return .success(data)
        case .failure(let error):
            return .failure(Self.failure(for: error))
        }
    }

    static func failure(for error: any Error) -> ReconFailure {
        switch error as? ReconInvocationFailure ?? .action(error) {
        case .invalidInput(let message):
            return ReconFailure("INVALID_ARGUMENT", "input", message)
        case .encoding(let message):
            return ReconFailure("INSPECTION_FAILED", "encoding", "Result could not be encoded: \(message)")
        case .action(let error as ReconActionError):
            switch error {
            case .invalidArgument(let message):
                return ReconFailure("INVALID_ARGUMENT", "target-execution", message)
            case .staleReference(let message):
                return ReconFailure("STALE_NODE", "target-execution", message)
            case .unavailable(let message):
                return ReconFailure("INSPECTION_UNAVAILABLE", "target-execution", message)
            case .cancelled:
                return ReconFailure(
                    "INSPECTION_TIMEOUT", "target-execution", "Action observed cancellation",
                    mayContinue: false)
            }
        case .action(let error):
            return ReconFailure(
                "INSPECTION_FAILED", "target-execution",
                "Action failed: \(String(describing: error).prefix(256))")
        }
    }
}

/// Shared between the waiting caller and the scheduled work, which may outlive the caller's wait.
private final class Execution: Sendable {
    private struct State {
        var started = false
        var answer: Result<Data, any Error>?
    }

    private let state = Mutex(State())

    var started: Bool { state.withLock { $0.started } }
    var answer: Result<Data, any Error>? { state.withLock { $0.answer } }

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
