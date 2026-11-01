import Foundation
import Testing

extension ProposalSDK {
    @Suite
    struct CodableBoundary {
        @Test func invalidInputIsRejectedBeforeTheBodyRuns() {
            let before = Calls.count
            let payloads = [
                #"{"name":"Mark","extra":1}"#,  // unknown member: JSONDecoder would ignore it
                #"{}"#,  // missing required member
                #"{"name":null}"#,  // null for a required member
                #"{"name":7}"#,  // wrong type
                #"["Mark"]"#,  // not an object
            ]
            for payload in payloads {
                let rejected = failure(invoke("demo", "greeting", payload))
                #expect(rejected?.code == "INVALID_ARGUMENT", "\(payload)")
                #expect(rejected?.stage == "input", "\(payload)")
            }
            let overflow = failure(
                invoke("demo", "measure", #"{"id":"a","ratio":1,"count":1,"window":4294967296}"#))
            #expect(overflow?.code == "INVALID_ARGUMENT")
            let explicitNull = failure(
                invoke("demo", "measure", #"{"id":"a","ratio":1,"count":1,"window":1,"note":null}"#))
            #expect(explicitNull?.code == "INVALID_ARGUMENT")
            #expect(Calls.count == before)
        }

        @Test func numbersKeepTheirPrecisionAndAbsenceStaysAbsent() throws {
            let measured = try #require(
                json(
                    invoke(
                        "demo", "measure",
                        #"{"id":"m","ratio":0.1,"count":9223372036854775807,"window":4294967295}"#)))
            let object = try #require(
                try JSONSerialization.jsonObject(with: Data(measured.utf8)) as? [String: Any])
            #expect(measured.contains("\"ratio\":0.1"))
            #expect(measured.contains("\"count\":9223372036854775807"))
            #expect(object["windowId"] as? UInt32 == 4_294_967_295)
            #expect(object["note"] == nil)
            #expect(object["ignored"] == nil)
        }

        @Test func defaultedParametersAreOptionalMembers() throws {
            let surveyed = try #require(
                json(invoke("demo", "survey", #"{"points":[[1,2],[3.5,4],[5,6]]}"#)))
            #expect(surveyed.contains(#""points":[[1,2],[3.5,4]]"#))
            #expect(surveyed.contains(#""completeness":"truncated-by-limit""#))
            #expect(surveyed.contains(#""maybe":["kept",null]"#))
            let all = try #require(json(invoke("demo", "survey", #"{"points":[[1,2]],"limit":5}"#)))
            #expect(all.contains(#""completeness":"complete""#))
        }

        @Test func unencodableResultsFailOnceWithoutReexecution() {
            let before = Calls.count
            let outcome = failure(invoke("demo", "unencodable", "{}"))
            #expect(outcome?.code == "INSPECTION_FAILED")
            #expect(outcome?.stage == "encoding")
            #expect(Calls.count == before + 1)
        }

        @Test func thrownErrorsKeepTheirClassification() {
            #expect(failure(invoke("demo", "fail", #"{"kind":"stale"}"#))?.code == "STALE_NODE")
            #expect(
                failure(invoke("demo", "fail", #"{"kind":"invalid"}"#))?.code == "INVALID_ARGUMENT")
            let unknown = failure(invoke("demo", "fail", #"{"kind":"other"}"#))
            #expect(unknown?.code == "INSPECTION_FAILED")
            #expect(unknown?.stage == "target-execution")
        }

        /// An action that observed cancellation has returned, so nothing is still running in
        /// the target.
        @Test func anActionThatObservedCancellationLeavesNothingRunning() {
            let cancelled = failure(invoke("demo", "fail", #"{"kind":"cancelled"}"#))
            #expect(cancelled?.code == "INSPECTION_TIMEOUT")
            #expect(cancelled?.stage == "target-execution")
            #expect(cancelled?.targetExecutionMayContinue == false)
        }

        /// An action reproducing a published error contract names the stage that contract names.
        @Test func thrownErrorsKeepTheStageTheyName() {
            let staged = failure(invoke("demo", "fail", #"{"kind":"staged"}"#))
            #expect(staged?.code == "INVALID_ARGUMENT")
            #expect(staged?.stage == "bounds")
            #expect(staged?.message == "bad depth")
            let unavailable = failure(invoke("demo", "fail", #"{"kind":"unavailable"}"#))
            #expect(unavailable?.code == "INSPECTION_UNAVAILABLE")
            #expect(unavailable?.stage == "registry")
        }

        @Test func voidActionsAnswerAnEmptyObject() {
            #expect(json(invoke("demo", "nothing", "{}")) == "{}")
        }

        /// The runtime does not re-parse results, so a result that contradicts its schema still
        /// reaches the wire. `ReconTestHost` is what catches it; see `TestHostTests`.
        @Test func resultsAreNotRevalidatedByTheRuntime() {
            #expect(json(invoke("demo", "misdeclared", "{}")) == #""bare""#)
        }
    }
}
