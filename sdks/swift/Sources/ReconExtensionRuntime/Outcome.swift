import Foundation
import ReconExtension

/// One invocation's failure as the host's error envelope publishes it.
///
/// `targetExecutionMayContinue` is reported only where the distinction exists: work that may
/// still be running in the target after the runtime stopped waiting for it.
public struct ReconFailure: Error, Sendable, Equatable {
    public let code: String
    public let stage: String
    public let message: String
    public let targetExecutionMayContinue: Bool?

    public init(code: String, stage: String, message: String, mayContinue: Bool? = nil) {
        self.code = code
        self.stage = stage
        self.message = message
        self.targetExecutionMayContinue = mayContinue
    }
}

public enum ReconOutcome: Sendable, Equatable {
    case success(Data)
    case failure(ReconFailure)
}

extension ReconFailure {
    /// Classifies what an action threw. A known typed error keeps its classification and the
    /// stage it named; anything else is a bounded `INSPECTION_FAILED`.
    static func classifying(_ error: any Error) -> ReconFailure {
        switch error as? ReconInvocationFailure ?? .action(error) {
        case .invalidInput(let message):
            ReconFailure(code: "INVALID_ARGUMENT", stage: "input", message: message)
        case .encoding(let message):
            ReconFailure(
                code: "INSPECTION_FAILED", stage: "encoding",
                message: "Result could not be encoded: \(message)")
        case .action(let error as ReconActionError):
            classifying(error)
        case .action(let error):
            ReconFailure(
                code: "INSPECTION_FAILED", stage: ReconActionError.defaultStage,
                message: "Action failed: \(String(describing: error).prefix(256))")
        }
    }

    private static func classifying(_ error: ReconActionError) -> ReconFailure {
        switch error {
        case .invalidArgument(let message, let stage):
            ReconFailure(code: "INVALID_ARGUMENT", stage: stage, message: message)
        case .staleReference(let message, let stage):
            ReconFailure(code: "STALE_NODE", stage: stage, message: message)
        case .unavailable(let message, let stage):
            ReconFailure(code: "INSPECTION_UNAVAILABLE", stage: stage, message: message)
        case .cancelled:
            ReconFailure(
                code: "INSPECTION_TIMEOUT", stage: ReconActionError.defaultStage,
                message: "Action observed cancellation")
        }
    }
}
