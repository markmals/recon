import Foundation
import Testing

@testable import ReconHelper

@Suite("The helper's transport")
struct ServerTransportTests {
    let helper: HelperFixture

    init() throws { helper = try HelperFixture() }

    @Test("A frame of no length is refused")
    func emptyFrame() throws {
        let answer = try decoded(helper.exchange(header: 0))
        #expect(answer.errorCode == "INVALID_ARGUMENT")
        #expect(answer.errorStage == "framing")
        #expect(answer.errorMessage == "Request exceeds 64 KiB or is empty")
    }

    @Test("A frame over 64 KiB is refused before its body is read")
    func oversizedFrame() throws {
        let answer = try decoded(helper.exchange(header: 65537))
        #expect(answer.errorCode == "INVALID_ARGUMENT")
        #expect(answer.errorStage == "framing")
    }

    @Test("A frame of exactly 64 KiB is served")
    func largestFrame() throws {
        var request = helper.request("open")
        var body = try JSONSerialization.data(withJSONObject: request)
        request["filler"] = String(repeating: "x", count: 65536 - body.count - "filler".count - 6)
        body = try JSONSerialization.data(withJSONObject: request)
        #expect(body.count == 65536)
        #expect(try decoded(helper.exchange(frame: body))["ok"] as? Bool == true)
    }

    @Test("A body that is not JSON is refused")
    func unparsableBody() throws {
        let answer = try decoded(helper.exchange(frame: Data("not json".utf8)))
        #expect(answer.errorCode == "INVALID_ARGUMENT")
        #expect(answer.errorStage == "json")
        #expect(answer.errorMessage == "Request must be one JSON object")
    }

    @Test("A body that is not one object is refused")
    func unexpectedBody() throws {
        let answer = try decoded(helper.exchange(frame: Data("[1,2]".utf8)))
        #expect(answer.errorCode == "INVALID_ARGUMENT")
        #expect(answer.errorStage == "json")
    }

    @Test("A peer that does not claim its own incarnation is refused")
    func peerMismatch() throws {
        var request = helper.request("open")
        request.removeValue(forKey: "clientPidversion")
        let answer = try helper.exchange(request)
        #expect(answer.errorCode == "STALE_SESSION")
        #expect(answer.errorStage == "peer")
        #expect(answer.errorMessage == "Peer process incarnation changed")

        request["clientPidversion"] = ReconProcessIncarnation.current.pidversion
        request["clientPid"] = 1
        #expect(try helper.exchange(request).errorCode == "STALE_SESSION")
    }

    @Test("A request without an operation is refused")
    func missingOperation() throws {
        var request = helper.request("open")
        request["op"] = 7
        let answer = try helper.exchange(request)
        #expect(answer.errorCode == "INVALID_ARGUMENT")
        #expect(answer.errorStage == "request")
        #expect(answer.errorMessage == "Operation is required")
    }

    @Test("A client that speaks the earlier wire protocol is refused")
    func protocolMismatch() throws {
        let answer = try helper.exchange(helper.request("open", ["protocol": 1]))
        #expect(answer.errorCode == "INSPECTION_PROTOCOL_MISMATCH")
        #expect(answer.errorStage == "protocol")
        #expect(answer.errorMessage == "Helper build or wire protocol differs")
    }

    @Test("A request for another helper build is refused")
    func buildMismatch() throws {
        let stale = helper.request("reconcile", ["helperBuild": "stale-helper-build"])
        #expect(try helper.exchange(stale).errorCode == "INSPECTION_PROTOCOL_MISMATCH")
    }

    @Test("An operation outside the closed set is refused")
    func unknownOperation() throws {
        let answer = try helper.exchange(helper.request("screenshot"))
        #expect(answer.errorCode == "INVALID_ARGUMENT")
        #expect(answer.errorStage == "operation")
        #expect(answer.errorMessage == "Unsupported closed operation")
    }

    private func decoded(_ answer: Data) throws -> [String: Any] {
        guard let members = try JSONSerialization.jsonObject(with: answer) as? [String: Any] else {
            throw FixtureFailure.malformedAnswer
        }
        return members
    }
}
