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
///
/// `stage` names the phase of the operation that failed. It defaults to the execution of the
/// action itself; an action reproducing a published error contract — as Recon's own AppKit
/// inspector does — passes the stage that contract names.
public enum ReconActionError: Error, Sendable, Equatable {
    case invalidArgument(_ message: String, stage: String = Self.defaultStage)
    case staleReference(_ message: String, stage: String = Self.defaultStage)
    case unavailable(_ message: String, stage: String = Self.defaultStage)
    case cancelled

    public static let defaultStage = "target-execution"
}
