import Foundation
import ReconExtensionRuntime
import Testing

@testable import ReconHelper

@Suite("Response envelopes")
struct EnvelopeTests {
    @Test("A refusal names its code, stage and message")
    func refusalMembers() throws {
        let envelope = ReconEnvelope.error(
            "STALE_SESSION", "read", "Session authentication failed")
        #expect(envelope["ok"] as? Bool == false)
        let error = try #require(envelope["error"] as? [String: Any])
        #expect(error["code"] as? String == "STALE_SESSION")
        #expect(error["stage"] as? String == "read")
        #expect(error["message"] as? String == "Session authentication failed")
    }

    @Test("`ok` goes on the wire as a JSON boolean, not as a number")
    func serializedBoolean() throws {
        let refusal = ReconEnvelope.error("STALE_SESSION", "read", "Session authentication failed")
        let json = String(decoding: try #require(ReconEnvelope.serialized(refusal)), as: UTF8.self)
        #expect(json.contains(#""ok":false"#))
    }

    @Test("An action's object is spliced in behind `ok`")
    func successSplice() {
        let answered = ReconEnvelope.response(.success(Data(#"{"windows":[]}"#.utf8)))
        #expect(String(decoding: answered, as: UTF8.self) == #"{"ok":true,"windows":[]}"#)
    }

    @Test("An empty action result leaves `ok` alone")
    func emptySplice() {
        let answered = ReconEnvelope.response(.success(Data("{}".utf8)))
        #expect(String(decoding: answered, as: UTF8.self) == #"{"ok":true}"#)
    }

    @Test("A result that is not an object is a refusal, not a malformed answer")
    func nonObjectResult() throws {
        let answered = ReconEnvelope.response(.success(Data("[]".utf8)))
        let error = try #require(decoded(answered)["error"] as? [String: Any])
        #expect(error["code"] as? String == "INSPECTION_FAILED")
        #expect(error["stage"] as? String == "encoding")
    }

    @Test("A failure reports targetExecutionMayContinue only when the runtime set it")
    func failureEnvelope() throws {
        let quiet = ReconFailure(code: "STALE_SESSION", stage: "read", message: "Gone")
        let noisy = ReconFailure(
            code: "INSPECTION_TIMEOUT", stage: "target-execution", message: "Late",
            mayContinue: true)
        let without = try #require(decoded(.failure(quiet))["error"] as? [String: Any])
        let with = try #require(decoded(.failure(noisy))["error"] as? [String: Any])
        #expect(without["targetExecutionMayContinue"] == nil)
        #expect(with["targetExecutionMayContinue"] as? Bool == true)
        #expect(with["message"] as? String == "Late")
    }

    private func decoded(_ outcome: ReconOutcome) -> [String: Any] {
        decoded(ReconEnvelope.response(outcome))
    }

    private func decoded(_ answered: Data) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: answered)) as? [String: Any] ?? [:]
    }
}
