import Foundation
import ReconExtensionRuntime

// A test file that imports the testing library writes extension declarations too.
@_exported import ReconExtension

/// Assembles extensions the way the helper does and runs invocations through the real runtime:
/// input validation, isolation, error classification, bounds, and encoding.
///
/// Every success it returns has been checked against the action's published output schema, which
/// the runtime itself does not re-parse.
public final class ReconTestHost {
    /// Long enough that a slow machine does not turn a correct action into a timeout, and short
    /// enough that a deadlocked one fails the test rather than hanging the suite.
    public static let deadline = Duration.seconds(10)

    private let runtime: ReconRuntime
    private let extensions: [ReconExtensionDescriptor]

    public init(_ extensions: ReconExtensionDescriptor...) throws {
        runtime = try ReconRuntime(assembling: extensions)
        self.extensions = extensions
    }

    /// Invokes one action. Call it from a test that is not `@MainActor`: it waits for the action,
    /// and a `@MainActor` action needs the main thread to be free.
    public func invoke(
        _ action: String, _ payload: String = "{}", in extensionIdentifier: String? = nil
    ) throws(ReconSchemaMismatch) -> ReconTestOutcome {
        let identifier = extensionIdentifier ?? declaring(action)
        let outcome = runtime.invoke(
            identifier, action, payload: Data(payload.utf8), access: .unrestricted,
            deadline: .now + Self.deadline)
        switch outcome {
        case .failure(let failure):
            return .failure(code: failure.code, stage: failure.stage, message: failure.message)
        case .success(let result):
            try check(result, of: action, in: identifier)
            return .success(result)
        }
    }

    private func check(_ result: Data, of action: String, in identifier: String)
        throws(ReconSchemaMismatch)
    {
        guard let descriptor = runtime.action(identifier, action),
            let violation = ReconSchemaValidator.violation(in: result, against: descriptor.output)
        else { return }
        throw ReconSchemaMismatch(
            extensionIdentifier: identifier, action: action, violation: violation)
    }

    /// Equal action names in different extensions are allowed, so an unqualified invocation
    /// names the first assembled extension that exports it.
    private func declaring(_ action: String) -> String {
        let owner = extensions.first { descriptor in
            descriptor.actions.contains { $0.name == action }
        }
        return (owner ?? extensions.first)?.identifier ?? ""
    }
}
