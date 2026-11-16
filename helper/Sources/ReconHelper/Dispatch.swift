import Foundation
import ReconExtensionRuntime

/// What every authenticated native invocation does the same way, whichever dispatcher names it.
///
/// Authentication, the single-flight guard and the session lock are the server's. Queueing, the
/// start gate, the deadline wait and every published observation are the runtime's and the
/// extension's. A read and an interaction take the same guard, so neither ever runs in the
/// target while the other does; which operations exist, and which extension answers them, is
/// the dispatcher's and is never widened here.
struct ReconSessionWork: Sendable {
    /// One invocation's own budget, independent of the transport's timeout.
    static let budget = Duration.seconds(2)

    private static let envelopeMembers = [
        "op", "protocol", "helperBuild", "sessionId", "capability", "clientPid",
        "clientPidversion", "clientUid",
    ]

    let lifecycle: ReconAccessLifecycle
    let runtime: ReconRuntime
    /// The extension that answers this request.
    let extensionIdentifier: String
    /// What kind of request this is, which is the stage a refusal of it names.
    let stage: String

    func answer(
        _ operation: String, _ request: [String: Any], startedAt: ContinuousClock.Instant
    ) -> Data? {
        guard let session = lifecycle.authenticated(request) else {
            return Self.refusal("STALE_SESSION", stage, "Session authentication failed")
        }
        let deadline = startedAt.advanced(by: Self.budget)
        guard ContinuousClock.now < deadline else {
            return Self.refusal(
                "INSPECTION_TIMEOUT", "transport",
                "Native request exceeded its deadline before dispatch")
        }
        guard lifecycle.beginRead() else {
            return Self.refusal(
                "INSPECTION_BUSY", "target-execution", "A prior native request has not finished")
        }
        let outcome = runtime.invoke(
            extensionIdentifier, operation, payload: Self.payload(request),
            access: access(for: session), deadline: deadline)
        return ReconEnvelope.response(outcome)
    }

    /// Everything the request carries that is not the action's own input.
    private static func payload(_ request: [String: Any]) -> Data {
        var parameters = request
        for member in envelopeMembers { parameters.removeValue(forKey: member) }
        return (try? JSONSerialization.data(withJSONObject: parameters)) ?? Data()
    }

    private func access(for session: String) -> ReconAccess {
        ReconAccess(
            begin: { [lifecycle] in lifecycle.isCurrent(session) },
            isRevoked: { [lifecycle] in !lifecycle.isCurrent(session) },
            settled: { [lifecycle] in lifecycle.endRead() })
    }

    static func refusal(_ code: String, _ stage: String, _ message: String) -> Data? {
        ReconEnvelope.serialized(ReconEnvelope.error(code, stage, message))
    }
}
