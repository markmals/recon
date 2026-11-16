import Foundation
import ReconExtensionRuntime

/// One native read: an invocation of one generated read for an authenticated session.
///
/// The read set is closed and observes only; nothing reached through it changes the target.
struct ReconReadDispatch: Sendable {
    /// The closed set of reads this helper serves, which is also the capability set a session
    /// is opened with.
    static let operations = ["windows", "tree", "node", "find", "fonts"]

    let lifecycle: ReconAccessLifecycle
    let runtime: ReconRuntime

    func answer(
        _ operation: String, _ request: [String: Any], startedAt: ContinuousClock.Instant
    ) -> Data? {
        ReconSessionWork(
            lifecycle: lifecycle, runtime: runtime, extensionIdentifier: "appkit", stage: "read"
        ).answer(operation, request, startedAt: startedAt)
    }
}
