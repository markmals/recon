import Foundation
import ReconExtension
import ReconNativeAccessors

/// The target process incarnation an observation was read from.
///
/// An absent bundle identifier or path is published as `null` rather than omitted: the member
/// always exists, and its absence is a fact about the process.
@ReconModel
public struct ReconProcessIdentity: Codable, Sendable, Equatable {
    public let pid: Int
    public let pidversion: UInt32
    public let uid: UInt32
    public let bundleId: ReconNullable<String>
    public let bundlePath: ReconNullable<String>
    public let executablePath: ReconNullable<String>
    public let architecture: String
    public let osBuild: String
    public let osVersion: String

    /// This process, as the helper's lifecycle response reports it.
    public static var current: ReconProcessIdentity {
        ReconProcessIdentity(ReconReadProcessIdentity())
    }

    init(_ facts: ReconProcessIdentityFacts) {
        pid = Int(facts.pid)
        pidversion = facts.pidversion
        uid = facts.uid
        bundleId = ReconNullable(facts.bundleId)
        bundlePath = ReconNullable(facts.bundlePath)
        executablePath = ReconNullable(facts.executablePath)
        architecture = facts.architecture
        osBuild = facts.osBuild
        osVersion = facts.osVersion
    }
}
