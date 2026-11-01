import Foundation
import ReconExtensionRuntime

/// One native read: an invocation of one generated action for an authenticated session.
///
/// Authentication, the single-flight guard and the session lock are the server's. Queueing, the
/// start gate, the deadline wait and every published observation are the runtime's and the
/// extension's.
struct ReconReadDispatch: Sendable {
    /// The closed set of reads this helper serves, which is also the capability set a session
    /// is opened with.
    static let operations = ["windows", "tree", "node", "find", "fonts"]
    /// A read's own budget, independent of the transport's timeout.
    static let budget = Duration.seconds(2)

    private static let envelopeMembers = [
        "op", "protocol", "helperBuild", "sessionId", "capability", "clientPid",
        "clientPidversion", "clientUid",
    ]

    let lifecycle: ReconAccessLifecycle
    let runtime: ReconRuntime

    func answer(
        _ operation: String, _ request: [String: Any], startedAt: ContinuousClock.Instant
    ) -> Data? {
        guard let session = lifecycle.authenticated(request) else {
            return refusal("STALE_SESSION", "read", "Session authentication failed")
        }
        let deadline = startedAt.advanced(by: Self.budget)
        guard ContinuousClock.now < deadline else {
            return refusal(
                "INSPECTION_TIMEOUT", "transport",
                "Native request exceeded its deadline before dispatch")
        }
        guard lifecycle.beginRead() else {
            return refusal(
                "INSPECTION_BUSY", "target-execution", "A prior native read has not finished")
        }
        let outcome = runtime.invoke(
            "appkit", operation, payload: payload(request), access: access(for: session),
            deadline: deadline)
        return ReconEnvelope.response(outcome)
    }

    /// Everything the request carries that is not the action's own input.
    private func payload(_ request: [String: Any]) -> Data {
        var parameters = request
        for member in Self.envelopeMembers { parameters.removeValue(forKey: member) }
        return (try? JSONSerialization.data(withJSONObject: parameters)) ?? Data()
    }

    private func access(for session: String) -> ReconAccess {
        ReconAccess(
            begin: { [lifecycle] in lifecycle.isCurrent(session) },
            isRevoked: { [lifecycle] in !lifecycle.isCurrent(session) },
            settled: { [lifecycle] in lifecycle.endRead() })
    }

    private func refusal(_ code: String, _ stage: String, _ message: String) -> Data? {
        ReconEnvelope.serialized(ReconEnvelope.error(code, stage, message))
    }
}
