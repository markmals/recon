import Foundation
import ReconExtension
import ReconNativeSupport

/// What the client asked this observation to cover.
@ReconModel
struct ObservationScope: Codable, Equatable {
    let root: ReconNullable<String>
    let windowId: ReconNullable<Int>
    let depth: Int
    let limit: Int
}

/// Where one observation came from: which session, which helper, which process, and what it
/// was asked to look at. Two observations of the same target are still two observations.
@ReconModel
struct Observation: Codable, Equatable {
    let observationId: String
    let source: String
    let sessionId: String
    let helperBuild: String
    let `protocol`: Int
    let identity: ReconProcessIdentity
    let scope: ObservationScope
    let fields: [String]
    let kind: String
    let consistency: String
    let layerState: String
    let startTime: Double
    let endTime: Double
}

extension Work {
    func observation(fields: [String], kind: String) -> Observation {
        Observation(
            observationId: Self.observationIdentifier(), source: "native-appkit",
            sessionId: session.identifier, helperBuild: session.helperBuild,
            protocol: session.protocolVersion, identity: .current,
            scope: ObservationScope(
                root: ReconNullable(request.root), windowId: ReconNullable(request.windowId),
                depth: depth, limit: limit),
            fields: fields, kind: kind, consistency: "main-thread-best-effort",
            layerState: "model", startTime: startedAt, endTime: Date.now.timeIntervalSince1970)
    }

    /// Observations are distinct evidence, so each one is named by its own random identifier
    /// rather than by what it observed.
    private static func observationIdentifier() -> String {
        var generator = SystemRandomNumberGenerator()
        return (0..<4).map { _ in String(format: "%016llx", generator.next()) }.joined()
    }
}
