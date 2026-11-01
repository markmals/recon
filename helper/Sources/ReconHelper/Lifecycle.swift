import Foundation
import ReconNativeSupport
import Synchronization

/// The access lifecycle: at most one open session, the capability that authenticates it, and
/// the single-flight guard one native read at a time takes.
///
/// One lock serializes every transition. An action's start gate takes the same lock, so a close
/// either wins and the action never begins, or it lands after the action committed and runs as
/// in-flight execution the close does not stop.
final class ReconAccessLifecycle: @unchecked Sendable {
    private struct Session {
        let identifier: String
        let capability: String
        let profile: String
        let establishedMode: String
    }

    private static let profiles = ["standard", "research"]
    private static let modes = ["existing", "launch", "attach"]

    private let lock = NSLock()
    private let reading = Atomic<Bool>(false)
    private let incarnation: ReconProcessIncarnation
    private let build: String
    private var active: Session?
    /// The last session this process revoked, so its owner learns that it closed rather than
    /// that it never existed.
    private var lastClosed: String?

    init(incarnation: ReconProcessIncarnation, build: String) {
        self.incarnation = incarnation
        self.build = build
    }

    func open(_ request: [String: Any]) -> [String: Any] {
        if let refusal = identityRefusal(request) { return refusal }
        guard let profile = request["profile"] as? String, Self.profiles.contains(profile),
            let mode = request["mode"] as? String, Self.modes.contains(mode)
        else {
            return ReconEnvelope.error(
                "INVALID_ARGUMENT", "open", "Requested profile and mode are required")
        }
        lock.lock()
        defer { lock.unlock() }
        if let refusal = reopenRefusal(request, profile: profile, mode: mode) { return refusal }
        guard let session = active else {
            guard let fresh = established(profile: profile, mode: mode) else {
                return ReconEnvelope.error(
                    "INSPECTION_FAILED", "random", "Secure randomness is unavailable")
            }
            return opened(fresh, reused: false, profile: profile, mode: mode)
        }
        return opened(session, reused: true, profile: profile, mode: mode)
    }

    func reconcile(_ request: [String: Any]) -> [String: Any] {
        guard let identifier = request["sessionId"] as? String, !identifier.isEmpty else {
            return ReconEnvelope.error(
                "INVALID_ARGUMENT", "reconcile", "Session identity is required")
        }
        lock.lock()
        defer { lock.unlock() }
        let current = active?.identifier == identifier
        return [
            "ok": true, "active": current,
            "outstandingRead": reading.load(ordering: .sequentiallyConsistent),
            "sessionId": current ? identifier : NSNull(), "incarnation": incarnationMembers,
        ]
    }

    func close(_ request: [String: Any]) -> [String: Any] {
        guard let identifier = request["sessionId"] as? String, !identifier.isEmpty else {
            return ReconEnvelope.error("INVALID_ARGUMENT", "close", "Session identity is required")
        }
        lock.lock()
        defer { lock.unlock() }
        if let session = active, session.identifier == identifier {
            guard session.capability == request["capability"] as? String else {
                return ReconEnvelope.error(
                    "STALE_SESSION", "close", "Session authentication failed")
            }
            lastClosed = identifier
            ReconNativeSession.close(identifier: identifier)
            active = nil
            return revocation(wasActive: true)
        }
        guard lastClosed == identifier else {
            return ReconEnvelope.error(
                "STALE_SESSION", "close", "Session is not known to this process")
        }
        return revocation(wasActive: false)
    }

    /// The session a read is authenticated for, or nil where it is not the current one.
    func authenticated(_ request: [String: Any]) -> String? {
        lock.lock()
        defer { lock.unlock() }
        guard let session = active, session.identifier == request["sessionId"] as? String,
            session.capability == request["capability"] as? String
        else { return nil }
        return session.identifier
    }

    func isCurrent(_ identifier: String) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return active?.identifier == identifier
    }

    func beginRead() -> Bool {
        reading.compareExchange(expected: false, desired: true, ordering: .sequentiallyConsistent)
            .exchanged
    }

    func endRead() {
        reading.store(false, ordering: .sequentiallyConsistent)
    }

    private func identityRefusal(_ request: [String: Any]) -> [String: Any]? {
        guard let pid = request["expectedPid"] as? NSNumber,
            let pidversion = request["expectedPidversion"] as? NSNumber,
            let uid = request["expectedUid"] as? NSNumber
        else {
            return ReconEnvelope.error(
                "INVALID_ARGUMENT", "identity", "Expected process identity is required")
        }
        guard pid.int32Value == incarnation.pid, pidversion.uint32Value == incarnation.pidversion,
            uid.uint32Value == incarnation.uid
        else {
            return ReconEnvelope.error(
                "STALE_SESSION", "identity", "Process incarnation or owner changed")
        }
        return nil
    }

    private func reopenRefusal(
        _ request: [String: Any], profile: String, mode: String
    ) -> [String: Any]? {
        guard let session = active else {
            guard request["sessionId"] == nil, request["capability"] == nil else {
                return ReconEnvelope.error(
                    "STALE_SESSION", "open", "Session authentication failed")
            }
            return nil
        }
        // A launch re-establishes the target, which an active session cannot survive.
        guard session.profile == profile, mode != "launch" else {
            return ReconEnvelope.error(
                "INSPECTION_BUSY", "open", "An incompatible inspection session is active")
        }
        guard session.identifier == request["sessionId"] as? String,
            session.capability == request["capability"] as? String
        else {
            return ReconEnvelope.error(
                "INSPECTION_BUSY", "open", "An inspection session is already active")
        }
        return nil
    }

    /// A fresh session's identity and its capability are independent secrets; the identity is
    /// half the width, because it is the one a client may publish.
    private func established(profile: String, mode: String) -> Session? {
        guard let identifier = reconToken()?.prefix(32), let capability = reconToken() else {
            return nil
        }
        let session = Session(
            identifier: String(identifier), capability: capability, profile: profile,
            establishedMode: mode)
        active = session
        ReconNativeSession.open(
            identifier: session.identifier, helperBuild: build,
            protocolVersion: ReconHelperIdentity.protocolVersion)
        return session
    }

    private func opened(
        _ session: Session, reused: Bool, profile: String, mode: String
    ) -> [String: Any] {
        var response: [String: Any] = [
            "ok": true, "sessionId": session.identifier, "reused": reused,
            "pid": incarnation.pid, "pidversion": incarnation.pidversion, "uid": incarnation.uid,
            "incarnation": incarnationMembers, "identity": identityMembers,
            "helperBuild": build, "protocol": ReconHelperIdentity.protocolVersion,
            "profile": session.profile, "mode": mode, "requestedMode": mode,
            "requestedProfile": profile, "establishedMode": session.establishedMode,
            "capabilities": ReconReadDispatch.operations,
            "effects": ["helper-bootstrap-resident"],
            "helperResident": true, "bootstrapActive": true,
        ]
        if !reused { response["capability"] = session.capability }
        return response
    }

    private func revocation(wasActive: Bool) -> [String: Any] {
        [
            "ok": true, "revoked": true, "wasActive": wasActive, "helperResident": true,
            "bootstrapActive": true,
        ]
    }

    private var incarnationMembers: [String: Any] {
        ["pid": incarnation.pid, "pidversion": incarnation.pidversion]
    }

    /// The identity the SDK reports, as the members the lifecycle response publishes. An absent
    /// bundle is `null` rather than missing: its absence is a fact about the process.
    private var identityMembers: [String: Any] {
        let identity = ReconProcessIdentity.current
        return [
            "pid": identity.pid, "pidversion": identity.pidversion, "uid": identity.uid,
            "bundleId": member(identity.bundleId.value),
            "bundlePath": member(identity.bundlePath.value),
            "executablePath": member(identity.executablePath.value),
            "architecture": identity.architecture, "osBuild": identity.osBuild,
            "osVersion": identity.osVersion,
        ]
    }

    private func member(_ text: String?) -> Any {
        if let text { return text }
        return NSNull()
    }
}
