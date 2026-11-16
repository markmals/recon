import Foundation
import ReconExtensionRuntime

/// The protocol gate and the closed operation set: what this build will answer at all.
struct ReconRouter: Sendable {
    let build: String
    let lifecycle: ReconAccessLifecycle
    let runtime: ReconRuntime

    func answer(_ request: [String: Any], startedAt: ContinuousClock.Instant) -> Data? {
        guard let operation = request["op"] as? String else {
            return refusal("INVALID_ARGUMENT", "request", "Operation is required")
        }
        guard speaksThisBuild(request) else {
            return refusal(
                "INSPECTION_PROTOCOL_MISMATCH", "protocol", "Helper build or wire protocol differs")
        }
        switch operation {
        case "open":
            return ReconEnvelope.serialized(lifecycle.open(request))
        case "reconcile":
            return ReconEnvelope.serialized(lifecycle.reconcile(request))
        case "close":
            return ReconEnvelope.serialized(lifecycle.close(request))
        case let read where ReconReadDispatch.operations.contains(read):
            return ReconReadDispatch(lifecycle: lifecycle, runtime: runtime)
                .answer(read, request, startedAt: startedAt)
        case let interaction where ReconInteractionDispatch.operations.contains(interaction):
            return ReconInteractionDispatch(lifecycle: lifecycle, runtime: runtime)
                .answer(interaction, request, startedAt: startedAt)
        default:
            return refusal("INVALID_ARGUMENT", "operation", "Unsupported closed operation")
        }
    }

    /// A client that was built against other code is refused rather than answered from this
    /// one: the two agree on what an observation means only when they are the same build.
    private func speaksThisBuild(_ request: [String: Any]) -> Bool {
        guard !build.isEmpty, let version = request["protocol"] as? NSNumber,
            version.intValue == ReconHelperIdentity.protocolVersion,
            let named = request["helperBuild"] as? String, named == build
        else { return false }
        return true
    }

    private func refusal(_ code: String, _ stage: String, _ message: String) -> Data? {
        ReconEnvelope.serialized(ReconEnvelope.error(code, stage, message))
    }
}
