import Foundation
import ReconExtension
import ReconNativeSupport

/// An Objective-C getter raised. The walker it replaces wrapped each facet in `@try`, so a
/// raising getter fails the facet that asked for it and nothing else; this error carries that
/// scope through Swift.
struct TargetGetterRaised: Error {}

/// One read's budget, coverage, and handles.
///
/// `visited` bounds the work the target does, `emitted` bounds what comes back, and the two
/// completeness lists explain every boundary the read hit.
final class Work {
    /// The most objects one read may touch, however it reaches them.
    static let visitLimit = 25_000
    /// The longest string any single value may carry, in Unicode scalars.
    static let stringLimit = 4096

    let request: InspectionRequest
    let session: ReconNativeSession
    let deadline: ContinuousClock.Instant
    let startedAt: Double

    private(set) var visited = 0
    var emitted = 0
    private(set) var narrowed: [String] = []
    private(set) var truncated: [String] = []
    private var exhausted = false

    init(request: InspectionRequest, session: ReconNativeSession, deadline: ContinuousClock.Instant)
    {
        self.request = request
        self.session = session
        self.deadline = deadline
        startedAt = Date.now.timeIntervalSince1970
    }

    var depth: Int { request.depth }
    var limit: Int { request.limit }
    var expired: Bool { ContinuousClock.now >= deadline }

    /// Spends one unit of the read's budget, or reports that there is none left.
    func consume() -> Bool {
        if expired { return false }
        if visited >= Self.visitLimit {
            note(truncated: "work:\(Self.visitLimit)")
            return false
        }
        visited += 1
        return true
    }

    func note(narrowed reason: String) {
        if !narrowed.contains(reason) { narrowed.append(reason) }
    }

    func note(truncated reason: String) {
        if !truncated.contains(reason) { truncated.append(reason) }
    }

    /// A handle for a live object. Running out of them ends the read rather than publishing a
    /// record that names nothing.
    func reference(_ object: AnyObject?) -> String? {
        guard let object else { return nil }
        guard let handle = session.handles.handle(for: object) else {
            exhausted = true
            note(truncated: "registry:\(ReconHandleRegistry.capacity)")
            return nil
        }
        return handle
    }

    func requiredReference(_ object: AnyObject?) throws -> String {
        guard let handle = reference(object) else { throw Self.registryExhausted }
        return handle
    }

    static let registryExhausted = ReconActionError.unavailable(
        "Native handle capacity exhausted", stage: "registry")

    /// A string bounded to what one value may carry, recording that it was cut.
    func bounded(_ text: String) -> String {
        let scalars = text.unicodeScalars
        guard scalars.count > Self.stringLimit else { return text }
        note(truncated: "string:\(Self.stringLimit)-scalars")
        return String(String.UnicodeScalarView(scalars.prefix(Self.stringLimit)))
    }

    func coverage() throws -> Coverage {
        if exhausted { throw Self.registryExhausted }
        return Coverage(
            visited: visited, returned: emitted, narrowed: narrowed, truncated: truncated,
            complete: truncated.isEmpty)
    }
}

/// What the read reached, and every boundary it stopped at.
@ReconModel
struct Coverage: Codable, Equatable {
    let visited: Int
    let returned: Int
    let narrowed: [String]
    let truncated: [String]
    let complete: Bool
}
