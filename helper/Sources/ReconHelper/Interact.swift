import Foundation
import ReconExtensionRuntime

/// One native interaction: an invocation of the one operation that changes the target.
///
/// Interaction is dispatched apart from reads on purpose. The read set is the capability set a
/// session is opened with and the set `ui view` reaches; serving this operation from the read
/// path would give a read path the power to change a target. It is reachable only from
/// `recon ui key` against a Mac Catalyst target, and only inside an open, authenticated
/// session — which is what makes a chord that runs the target's own code something a person
/// asked for.
struct ReconInteractionDispatch: Sendable {
    /// The closed set of interactions this helper serves. Nothing else changes a target, and
    /// this set is deliberately not published as a session capability: an interaction is not
    /// something a client selects, it is what one command reaches.
    static let operations = ["makeWindowKey"]

    let lifecycle: ReconAccessLifecycle
    let runtime: ReconRuntime

    func answer(
        _ operation: String, _ request: [String: Any], startedAt: ContinuousClock.Instant
    ) -> Data? {
        ReconSessionWork(
            lifecycle: lifecycle, runtime: runtime, extensionIdentifier: "catalyst",
            stage: "interaction"
        ).answer(operation, request, startedAt: startedAt)
    }
}
