import Foundation

/// Runtime-supplied scope for one invocation: its deadline and cooperative cancellation.
///
/// This is not caller input. A loop observes it; nothing can interrupt a getter already running.
public final class ReconContext: Sendable {
    public let deadline: ContinuousClock.Instant
    private let revoked: @Sendable () -> Bool

    package init(deadline: ContinuousClock.Instant, revoked: @escaping @Sendable () -> Bool) {
        self.deadline = deadline
        self.revoked = revoked
    }

    public var isCancelled: Bool { revoked() || ContinuousClock.now >= deadline }

    public func checkCancellation() throws(ReconActionError) {
        if isCancelled { throw .cancelled }
    }
}

/// Failures an action reports with an existing typed classification. Any other thrown error is
/// reported as `INSPECTION_FAILED`.
public enum ReconActionError: Error, Sendable, Equatable {
    case invalidArgument(String)
    case staleReference(String)
    case unavailable(String)
    case cancelled
}

/// How the runtime must schedule an action so its Swift isolation is preserved.
public enum ReconInvocation: Sendable {
    case nonisolated(@Sendable (Data, ReconContext) throws -> Data)
    case nonisolatedAsync(@Sendable (Data, ReconContext) async throws -> Data)
    case mainActor(@MainActor @Sendable (Data, ReconContext) throws -> Data)
    case mainActorAsync(@MainActor @Sendable (Data, ReconContext) async throws -> Data)
}

public struct ReconActionDescriptor: Sendable {
    public let name: String
    public let input: ReconSchemaNode
    public let output: ReconSchemaNode
    public let invocation: ReconInvocation

    public init(
        name: String, input: ReconSchemaNode, output: ReconSchemaNode, invocation: ReconInvocation
    ) {
        self.name = name
        self.input = input
        self.output = output
        self.invocation = invocation
    }
}

public struct ReconExtensionDescriptor: Sendable {
    public let identifier: String
    public let actions: [ReconActionDescriptor]

    public init(identifier: String, actions: [ReconActionDescriptor]) {
        self.identifier = identifier
        self.actions = actions
    }
}

/// Which phase of an invocation failed. Generated adapters keep the phases apart so the runtime
/// can report invalid input, an action's own error, and an unencodable result truthfully.
public enum ReconInvocationFailure: Error {
    case invalidInput(String)
    case action(any Error)
    case encoding(String)
}

/// The codec boundary generated adapters call. Settings live here, not in extension code.
public enum ReconCodec {
    public static func decode<T: Decodable>(_ type: T.Type, from payload: Data)
        throws(ReconInvocationFailure) -> T
    {
        do {
            return try JSONDecoder().decode(type, from: payload)
        } catch {
            throw .invalidInput(String(describing: error).prefix(256).description)
        }
    }

    public static func perform<R>(_ body: () throws -> R) throws(ReconInvocationFailure) -> R {
        do { return try body() } catch { throw .action(error) }
    }

    public static func performAsync<R>(_ body: () async throws -> R)
        async throws(ReconInvocationFailure) -> R
    {
        do { return try await body() } catch { throw .action(error) }
    }

    public static func encode<R: Encodable>(_ result: R) throws(ReconInvocationFailure) -> Data {
        do {
            return try JSONEncoder().encode(result)
        } catch {
            throw .encoding(String(describing: error).prefix(256).description)
        }
    }

    public static func encodeEmpty() -> Data { Data("{}".utf8) }
}
