import Foundation
import ReconExtension

public enum ReconAssemblyError: Error, Equatable {
    case duplicateExtension(String)
    case duplicateAction(extension: String, action: String)
}

/// The assembled, closed set of actions and the scheduling that preserves their isolation.
///
/// The runtime validates every input against its published schema before decoding it. It does
/// not re-parse results: an action's result conformance is a test obligation, which
/// `ReconExtensionTesting` discharges.
public final class ReconRuntime: Sendable {
    public static let maximumResultBytes = 8 * 1024 * 1024

    private let actions: [String: [String: ReconActionDescriptor]]

    public init(assembling extensions: [ReconExtensionDescriptor]) throws(ReconAssemblyError) {
        var assembled: [String: [String: ReconActionDescriptor]] = [:]
        for descriptor in extensions {
            guard assembled[descriptor.identifier] == nil else {
                throw .duplicateExtension(descriptor.identifier)
            }
            var named: [String: ReconActionDescriptor] = [:]
            for action in descriptor.actions {
                guard named[action.name] == nil else {
                    throw .duplicateAction(extension: descriptor.identifier, action: action.name)
                }
                named[action.name] = action
            }
            assembled[descriptor.identifier] = named
        }
        actions = assembled
    }

    public func action(_ extensionIdentifier: String, _ name: String) -> ReconActionDescriptor? {
        actions[extensionIdentifier]?[name]
    }

    /// Validates, schedules, and awaits one invocation. Blocks the caller; never call it on the
    /// main thread, which main-actor actions need.
    public func invoke(
        _ extensionIdentifier: String, _ name: String, payload: Data, access: ReconAccess,
        deadline: ContinuousClock.Instant
    ) -> ReconOutcome {
        guard let action = action(extensionIdentifier, name) else {
            access.settled()
            return .failure(
                ReconFailure(
                    code: "INVALID_ARGUMENT", stage: "operation",
                    message: "Unsupported closed operation"))
        }
        if let violation = ReconSchemaValidator.violation(in: payload, against: action.input) {
            access.settled()
            return .failure(
                ReconFailure(code: "INVALID_ARGUMENT", stage: "input", message: violation))
        }
        let execution = Execution()
        let done = DispatchSemaphore(value: 0)
        schedule(action.invocation, payload, access, deadline, execution) {
            access.settled()
            done.signal()
        }
        return answer(done, until: deadline, execution, action, access)
    }

    private func answer(
        _ done: DispatchSemaphore, until deadline: ContinuousClock.Instant, _ execution: Execution,
        _ action: ReconActionDescriptor, _ access: ReconAccess
    ) -> ReconOutcome {
        let queue = Self.queueStage(action.invocation)
        if done.wait(timeout: .now() + Self.remaining(until: deadline)) == .timedOut {
            let started = execution.started
            return .failure(
                ReconFailure(
                    code: "INSPECTION_TIMEOUT", stage: started ? "target-execution" : queue,
                    message: "Action exceeded its deadline", mayContinue: started))
        }
        guard let answer = execution.answer else {
            return .failure(
                access.isRevoked()
                    ? ReconFailure(
                        code: "STALE_SESSION", stage: queue,
                        message: "Access was revoked before the action started")
                    : ReconFailure(
                        code: "INSPECTION_TIMEOUT", stage: queue,
                        message: "Queued action expired before it started"))
        }
        switch answer {
        case .success(let result) where result.count > Self.maximumResultBytes:
            return .failure(
                ReconFailure(
                    code: "INSPECTION_UNAVAILABLE", stage: "response-size",
                    message: "Result exceeds 8 MiB"))
        case .success(let result):
            return .success(result)
        case .failure(let error):
            return .failure(.classifying(error))
        }
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

    /// Where an action that has not started yet is waiting. A main-actor action is queued on the
    /// target's main thread, which the published AppKit contract names explicitly.
    private static func queueStage(_ invocation: ReconInvocation) -> String {
        switch invocation {
        case .mainActor, .mainActorAsync: "main-thread-queue"
        case .nonisolated, .nonisolatedAsync: "queue"
        }
    }

    private static func remaining(until deadline: ContinuousClock.Instant) -> DispatchTimeInterval {
        let remaining = max(ContinuousClock.now.duration(to: deadline), .zero)
        let nanoseconds =
            remaining.components.seconds * 1_000_000_000
            + remaining.components.attoseconds / 1_000_000_000
        return .nanoseconds(Int(nanoseconds))
    }
}
