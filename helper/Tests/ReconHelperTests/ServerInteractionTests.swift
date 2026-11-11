import Foundation
import Testing

@testable import ReconHelper

/// The one operation that changes a target: who may reach it, and what reaches it instead of a
/// read. One native session is open per process, so these take their turn.
@Suite("The helper's interaction operation", .serialized)
struct ServerInteractionTests {
    let helper: HelperFixture

    init() throws { helper = try HelperFixture() }

    @Test("Interaction is a closed set the read set does not contain")
    func keepsReadsAndInteractionsApart() {
        #expect(ReconInteractionDispatch.operations == ["makeWindowKey"])
        #expect(
            Set(ReconReadDispatch.operations)
                .isDisjoint(with: ReconInteractionDispatch.operations))
    }

    @Test("A session is opened with the reads as its capabilities, never the interaction")
    func publishesOnlyReadsAsCapabilities() throws {
        let opened = try helper.exchange(helper.request("open"))
        let capabilities = opened["capabilities"] as? [String]
        #expect(capabilities == ReconReadDispatch.operations)
        #expect(capabilities?.contains("makeWindowKey") == false)
    }

    @Test("An interaction without an open session is refused")
    func refusesAnUnauthenticatedInteraction() throws {
        let request = helper.read("makeWindowKey", [:], ["windowId": 42])
        let answer = try helper.exchange(request)
        #expect(answer.errorCode == "STALE_SESSION")
        #expect(answer.errorStage == "interaction")
        #expect(answer.errorMessage == "Session authentication failed")
    }

    @Test("An interaction with another session's credentials is refused")
    func refusesAnotherSession() throws {
        _ = try helper.exchange(helper.request("open"))
        let forged = ["sessionId": String(repeating: "a", count: 32), "capability": "b"]
        let answer = try helper.exchange(helper.read("makeWindowKey", forged, ["windowId": 42]))
        #expect(answer.errorCode == "STALE_SESSION")
        #expect(answer.errorStage == "interaction")
    }

    @Test("An interaction after the session closed is refused")
    func refusesAnInteractionAfterClose() throws {
        let opened = try helper.exchange(helper.request("open"))
        let session = credentials(of: opened)
        _ = try helper.exchange(helper.request("close", session))
        let answer = try helper.exchange(helper.read("makeWindowKey", session, ["windowId": 42]))
        #expect(answer.errorCode == "STALE_SESSION")
        #expect(answer.errorStage == "interaction")
    }

    @Test("An authenticated interaction reaches the Catalyst extension, not a read")
    func dispatchesToTheCatalystExtension() throws {
        let opened = try helper.exchange(helper.request("open"))
        let answer = try helper.exchange(
            helper.read("makeWindowKey", credentials(of: opened), ["windowId": 42]))
        // This test process links AppKit and no UIKit, which is what the helper is inside an
        // AppKit target: the extension refuses rather than messaging something else.
        #expect(answer.errorCode == "INSPECTION_UNAVAILABLE")
        #expect(answer.errorMessage?.contains("not Mac Catalyst") == true)
    }

    @Test("An interaction whose input the action does not publish is refused")
    func refusesAMalformedInteraction() throws {
        let opened = try helper.exchange(helper.request("open"))
        let session = credentials(of: opened)
        let typed = try helper.exchange(
            helper.read("makeWindowKey", session, ["windowId": "42"]))
        #expect(typed.errorCode == "INVALID_ARGUMENT")
        let absent = try helper.exchange(helper.read("makeWindowKey", session))
        #expect(absent.errorCode == "INVALID_ARGUMENT")
    }

    @Test("An interaction on the earlier wire protocol never reaches the session")
    func refusesAnEarlierProtocol() throws {
        let opened = try helper.exchange(helper.request("open"))
        var request = helper.read("makeWindowKey", credentials(of: opened), ["windowId": 42])
        request["protocol"] = 1
        let answer = try helper.exchange(request)
        #expect(answer.errorCode == "INSPECTION_PROTOCOL_MISMATCH")
        #expect(answer.errorStage == "protocol")
    }

    private func credentials(of opened: [String: Any]) -> [String: Any] {
        ["sessionId": opened["sessionId"] ?? "", "capability": opened["capability"] ?? ""]
    }
}
