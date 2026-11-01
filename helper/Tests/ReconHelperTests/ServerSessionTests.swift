import Darwin
import Foundation
import Testing

@testable import ReconHelper

/// One native session is open per process, so these tests take their turn: a session opened by
/// one of them would revoke another's handles.
@Suite("The helper's access lifecycle", .serialized)
struct ServerSessionTests {
    let helper: HelperFixture

    init() throws { helper = try HelperFixture() }

    @Test("An open session answers with its identity, capability and effects")
    func opensASession() throws {
        let opened = try helper.exchange(helper.request("open"))
        #expect(opened["ok"] as? Bool == true)
        #expect((opened["sessionId"] as? String)?.count == 32)
        #expect((opened["capability"] as? String)?.count == 64)
        #expect(opened["reused"] as? Bool == false)
        #expect(opened["pid"] as? Int32 == getpid())
        #expect(opened["uid"] as? UInt32 == geteuid())
        #expect(opened["pidversion"] as? UInt32 == ReconProcessIncarnation.current.pidversion)
        #expect(opened["helperBuild"] as? String == "fixture")
        #expect(opened["protocol"] as? Int == 1)
        #expect(opened["profile"] as? String == "standard")
        #expect(opened["mode"] as? String == "existing")
        #expect(opened["requestedMode"] as? String == "existing")
        #expect(opened["requestedProfile"] as? String == "standard")
        #expect(opened["establishedMode"] as? String == "existing")
        #expect(opened["capabilities"] as? [String] == ["windows", "tree", "node", "find", "fonts"])
        #expect(opened["effects"] as? [String] == ["helper-bootstrap-resident"])
        #expect(opened["helperResident"] as? Bool == true)
        #expect(opened["bootstrapActive"] as? Bool == true)
        let incarnation = try #require(opened["incarnation"] as? [String: Any])
        #expect(incarnation["pid"] as? Int32 == getpid())
        #expect(incarnation["pidversion"] as? UInt32 == ReconProcessIncarnation.current.pidversion)
    }

    @Test("An open session publishes the process identity the SDK reports")
    func publishesItsIdentity() throws {
        let opened = try helper.exchange(helper.request("open"))
        let identity = try #require(opened["identity"] as? [String: Any])
        #expect(identity["pid"] as? Int32 == getpid())
        #expect(identity["uid"] as? UInt32 == geteuid())
        #expect(identity["pidversion"] as? UInt32 == ReconProcessIncarnation.current.pidversion)
        #expect(identity["architecture"] is String)
        #expect(identity["osBuild"] is String)
        #expect(identity["osVersion"] is String)
        #expect(identity["bundleId"] != nil)
        #expect(identity["bundlePath"] != nil)
        #expect(identity["executablePath"] != nil)
    }

    @Test("Re-opening the same session reuses it and issues no new capability")
    func reopensTheSameSession() throws {
        let opened = try helper.exchange(helper.request("open"))
        let again = try helper.exchange(helper.request("open", credentials(of: opened)))
        #expect(again["reused"] as? Bool == true)
        #expect(again["sessionId"] as? String == opened["sessionId"] as? String)
        #expect(again["capability"] == nil)
    }

    @Test("An open without the active session's credentials is refused")
    func refusesAnotherClient() throws {
        _ = try helper.exchange(helper.request("open"))
        let answer = try helper.exchange(helper.request("open"))
        #expect(answer.errorCode == "INSPECTION_BUSY")
        #expect(answer.errorStage == "open")
        #expect(answer.errorMessage == "An inspection session is already active")
    }

    @Test("An open with the wrong capability is refused")
    func refusesAWrongCapability() throws {
        let opened = try helper.exchange(helper.request("open"))
        var credentials = credentials(of: opened)
        credentials["capability"] = "invalid"
        #expect(try helper.exchange(helper.request("open", credentials)).errorCode
            == "INSPECTION_BUSY")
    }

    @Test("An open for another profile is refused while a session is active")
    func refusesAnIncompatibleProfile() throws {
        let opened = try helper.exchange(helper.request("open"))
        var request = credentials(of: opened)
        request["profile"] = "research"
        let answer = try helper.exchange(helper.request("open", request))
        #expect(answer.errorCode == "INSPECTION_BUSY")
        #expect(answer.errorMessage == "An incompatible inspection session is active")
    }

    @Test("An open that names a session nobody holds is refused")
    func refusesAnUnknownSession() throws {
        let answer = try helper.exchange(helper.request("open", ["sessionId": "unknown"]))
        #expect(answer.errorCode == "STALE_SESSION")
        #expect(answer.errorStage == "open")
        #expect(answer.errorMessage == "Session authentication failed")
    }

    @Test("An open that expects another incarnation or owner is refused")
    func refusesAnotherIncarnation() throws {
        let stale = try helper.exchange(helper.request("open", ["expectedPid": 1]))
        #expect(stale.errorCode == "STALE_SESSION")
        #expect(stale.errorStage == "identity")
        var missing = helper.request("open")
        missing.removeValue(forKey: "expectedUid")
        let answer = try helper.exchange(missing)
        #expect(answer.errorCode == "INVALID_ARGUMENT")
        #expect(answer.errorStage == "identity")
    }

    @Test("An open without a known profile and mode is refused")
    func refusesAnUnknownProfile() throws {
        let answer = try helper.exchange(helper.request("open", ["profile": "forensic"]))
        #expect(answer.errorCode == "INVALID_ARGUMENT")
        #expect(answer.errorStage == "open")
        #expect(answer.errorMessage == "Requested profile and mode are required")
    }

    @Test("Reconcile reports whether the session is active and whether a read is outstanding")
    func reconcilesTheSession() throws {
        let opened = try helper.exchange(helper.request("open"))
        let identifier = try #require(opened["sessionId"] as? String)
        let active = try helper.exchange(helper.request("reconcile", ["sessionId": identifier]))
        #expect(active["ok"] as? Bool == true)
        #expect(active["active"] as? Bool == true)
        #expect(active["outstandingRead"] as? Bool == false)
        #expect(active["sessionId"] as? String == identifier)
        let incarnation = try #require(active["incarnation"] as? [String: Any])
        #expect(incarnation["pid"] as? Int32 == getpid())

        let other = try helper.exchange(helper.request("reconcile", ["sessionId": "unknown"]))
        #expect(other["active"] as? Bool == false)
        #expect(other["sessionId"] is NSNull)
    }

    @Test("Closing revokes the session, and closing it again reports it was not active")
    func closesTheSession() throws {
        let opened = try helper.exchange(helper.request("open"))
        let closed = try helper.exchange(helper.request("close", credentials(of: opened)))
        #expect(closed["ok"] as? Bool == true)
        #expect(closed["revoked"] as? Bool == true)
        #expect(closed["wasActive"] as? Bool == true)
        #expect(closed["helperResident"] as? Bool == true)
        #expect(closed["bootstrapActive"] as? Bool == true)

        let again = try helper.exchange(helper.request("close", credentials(of: opened)))
        #expect(again["ok"] as? Bool == true)
        #expect(again["wasActive"] as? Bool == false)

        let identifier = try #require(opened["sessionId"] as? String)
        let reconciled = try helper.exchange(
            helper.request("reconcile", ["sessionId": identifier]))
        #expect(reconciled["active"] as? Bool == false)
    }

    @Test("Closing a session this process never opened is refused")
    func refusesAnUnknownClose() throws {
        let answer = try helper.exchange(
            helper.request("close", ["sessionId": "unknown", "capability": "unknown"]))
        #expect(answer.errorCode == "STALE_SESSION")
        #expect(answer.errorStage == "close")
        #expect(answer.errorMessage == "Session is not known to this process")
    }

    @Test("Closing with the wrong capability is refused")
    func refusesAWrongCapabilityOnClose() throws {
        let opened = try helper.exchange(helper.request("open"))
        var request = credentials(of: opened)
        request["capability"] = "invalid"
        let answer = try helper.exchange(helper.request("close", request))
        #expect(answer.errorCode == "STALE_SESSION")
        #expect(answer.errorMessage == "Session authentication failed")
    }

    @Test("A fresh session after a close shares nothing with the one before it")
    func opensAFreshSession() throws {
        let opened = try helper.exchange(helper.request("open"))
        _ = try helper.exchange(helper.request("close", credentials(of: opened)))
        let next = try helper.exchange(helper.request("open"))
        #expect(next["ok"] as? Bool == true)
        #expect(next["sessionId"] as? String != opened["sessionId"] as? String)
        #expect(next["capability"] as? String != opened["capability"] as? String)
    }

    @Test("A read the runtime answers comes back as one flat object")
    func dispatchesARead() throws {
        let opened = try helper.exchange(helper.request("open"))
        let answer = try helper.exchange(helper.read("windows", credentials(of: opened)))
        #expect(answer["ok"] as? Bool == true)
        #expect(answer["error"] == nil)
        #expect(answer["windows"] != nil)
    }

    @Test("A read of a malformed root or window identifier is refused")
    func refusesMalformedReads() throws {
        let opened = try helper.exchange(helper.request("open"))
        let session = credentials(of: opened)
        let root = try helper.exchange(helper.read("node", session, ["root": 123]))
        #expect(root.errorCode == "INVALID_ARGUMENT")
        let window = try helper.exchange(helper.read("tree", session, ["windowId": "123"]))
        #expect(window.errorCode == "INVALID_ARGUMENT")
    }

    @Test("A read after the session closed is refused")
    func refusesAReadAfterClose() throws {
        let opened = try helper.exchange(helper.request("open"))
        _ = try helper.exchange(helper.request("close", credentials(of: opened)))
        let request = helper.read("node", credentials(of: opened), ["root": "n:invalid"])
        let answer = try helper.exchange(request)
        #expect(answer.errorCode == "STALE_SESSION")
        #expect(answer.errorStage == "read")
        #expect(answer.errorMessage == "Session authentication failed")
    }

    private func credentials(of opened: [String: Any]) -> [String: Any] {
        ["sessionId": opened["sessionId"] ?? "", "capability": opened["capability"] ?? ""]
    }
}
