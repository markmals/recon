import Foundation
import ReconExtension
import ReconExtensionRuntime
import Synchronization
import Testing

final class Gate: Sendable {
    let revoked = Atomic(false)
    let began = Atomic(0)

    var access: ReconAccess {
        ReconAccess(
            begin: { [self] in
                guard !revoked.load(ordering: .sequentiallyConsistent) else { return false }
                began.add(1, ordering: .sequentiallyConsistent)
                return true
            },
            isRevoked: { [self] in revoked.load(ordering: .sequentiallyConsistent) })
    }
}

let runtime = try! ReconRuntime(
    assembling: [Demo.reconExtension, MainDemo.reconExtension, Other.reconExtension])

func invoke(
    _ extensionID: String, _ name: String, _ payload: String, gate: Gate = Gate(),
    within duration: Duration = .seconds(2)
) -> ReconOutcome {
    runtime.invoke(
        extensionID, name, payload: Data(payload.utf8), access: gate.access,
        deadline: .now + duration)
}

func json(_ outcome: ReconOutcome) -> String? {
    guard case .success(let data) = outcome else { return nil }
    return String(decoding: data, as: UTF8.self)
}

func failure(_ outcome: ReconOutcome) -> ReconFailure? {
    guard case .failure(let failure) = outcome else { return nil }
    return failure
}

/// Blocks the main thread until released, so main-actor work queues behind it.
final class MainThreadBlocker: Sendable {
    private let release = DispatchSemaphore(value: 0)

    init() {
        let entered = DispatchSemaphore(value: 0)
        DispatchQueue.main.async { [release] in
            entered.signal()
            release.wait()
        }
        entered.wait()
    }

    func unblock() { release.signal() }
}

func inBackground(_ body: @escaping @Sendable () -> ReconOutcome) -> () -> ReconOutcome {
    let box = Mutex<ReconOutcome?>(nil)
    let done = DispatchSemaphore(value: 0)
    Thread.detachNewThread {
        let outcome = body()
        box.withLock { $0 = outcome }
        done.signal()
    }
    return {
        done.wait()
        return box.withLock { $0! }
    }
}

/// One serialized parent: the suites share invocation counters and the main thread.
@Suite(.serialized)
struct ProposalSDK {}

extension ProposalSDK {
@Suite
struct ExportAssembly {
    @Test func bareAnnotationsExportInferredNamesInDeclarationOrder() {
        #expect(Demo.reconExtension.identifier == "demo")
        #expect(
            Demo.reconExtension.actions.map(\.name) == [
                "greeting", "measure", "survey", "fail", "unencodable", "misdeclared", "pause",
                "nothing", "bulk",
            ])
    }

    @Test func equalLocalNamesInDistinctExtensionsAreDistinctActions() {
        #expect(json(invoke("demo", "greeting", #"{"name":"Mark"}"#)) == #"{"message":"Hello, Mark"}"#)
        #expect(json(invoke("other", "greeting", #"{"name":"Mark"}"#)) == #"{"message":"Hi, Mark"}"#)
    }

    @Test func duplicateExtensionIdentifiersAreRejectedAtAssembly() {
        #expect(throws: ReconAssemblyError.duplicateExtension("demo")) {
            try ReconRuntime(assembling: [Demo.reconExtension, Demo.reconExtension])
        }
    }

    @Test func undeclaredAndUnannotatedNamesAreNotInvocable() {
        #expect(failure(invoke("demo", "helper", "{}"))?.code == "INVALID_ARGUMENT")
        #expect(failure(invoke("demo", "helper", "{}"))?.stage == "operation")
        #expect(failure(invoke("missing", "greeting", "{}"))?.stage == "operation")
    }
}
}

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
            invoke(
                "demo", "measure",
                #"{"id":"a","ratio":1,"count":1,"window":4294967296}"#))
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

    @Test func aResultThatContradictsItsSchemaIsNotPublished() {
        let outcome = failure(invoke("demo", "misdeclared", "{}"))
        #expect(outcome?.code == "INSPECTION_FAILED")
        #expect(outcome?.message.contains("published schema") == true)
    }

    @Test func thrownErrorsKeepTheirClassification() {
        #expect(failure(invoke("demo", "fail", #"{"kind":"stale"}"#))?.code == "STALE_NODE")
        #expect(failure(invoke("demo", "fail", #"{"kind":"invalid"}"#))?.code == "INVALID_ARGUMENT")
        let unknown = failure(invoke("demo", "fail", #"{"kind":"other"}"#))
        #expect(unknown?.code == "INSPECTION_FAILED")
        #expect(unknown?.stage == "target-execution")
    }

    @Test func voidActionsAnswerAnEmptyObject() {
        #expect(json(invoke("demo", "nothing", "{}")) == "{}")
    }
}
}

extension ProposalSDK {
@Suite
struct GeneratedSchemas {
    func schema(_ extensionID: String, _ name: String) -> (input: String, output: String) {
        let action = runtime.action(extensionID, name)!
        return (action.input.jsonSchema, action.output.jsonSchema)
    }

    @Test func synthesizedModelsFollowCodingKeysAndOptionality() {
        #expect(
            schema("demo", "measure").output
                == #"{"type":"object","properties":{"id":{"type":"string"},"ratio":{"type":"number"},"count":{"type":"integer","minimum":-9223372036854775808,"maximum":9223372036854775807},"windowId":{"type":"integer","minimum":0,"maximum":4294967295},"note":{"type":"string"}},"required":["id","ratio","count","windowId"],"additionalProperties":false}"#
        )
    }

    @Test func parametersBecomeAStrictInputObject() {
        #expect(
            schema("demo", "survey").input
                == #"{"type":"object","properties":{"points":{"type":"array","items":{"type":"array","items":{"type":"number"}}},"limit":{"type":"integer","minimum":-9223372036854775808,"maximum":9223372036854775807}},"required":["points"],"additionalProperties":false}"#
        )
    }

    @Test func customCodecsUseTheirStatedSchemaAndEnumsTheirRawValues() {
        #expect(
            schema("demo", "survey").output
                == #"{"type":"object","properties":{"points":{"type":"array","items":{"type":"array","items":{"type":"number"}}},"completeness":{"enum":["complete","truncated-by-limit"]},"tags":{"type":"object","additionalProperties":{"type":"integer","minimum":-9223372036854775808,"maximum":9223372036854775807}},"maybe":{"type":"array","items":{"anyOf":[{"type":"string"},{"type":"null"}]}}},"required":["points","completeness","tags","maybe"],"additionalProperties":false}"#
        )
    }
}
}

extension ProposalSDK {
@Suite
struct IsolationAndLifecycle {
    @Test func mainActorActionsRunOnTheMainThreadAndOthersDoNot() {
        #expect(json(invoke("main-demo", "whereAmI", "{}"))?.contains(#""mainThread":true"#) == true)
        #expect(json(invoke("main-demo", "elsewhere", "{}"))?.contains(#""mainThread":false"#) == true)
    }

    @Test func revocationBeforeAQueuedActionStartsPreventsItFromRunning() {
        let gate = Gate()
        let callsBefore = MainDemo.callsSnapshot()
        let blocker = MainThreadBlocker()
        let pending = inBackground { invoke("main-demo", "whereAmI", "{}", gate: gate) }
        usleep(50_000)
        gate.revoked.store(true, ordering: .sequentiallyConsistent)
        blocker.unblock()
        let outcome = failure(pending())
        #expect(outcome?.code == "STALE_SESSION")
        #expect(outcome?.stage == "queue")
        #expect(gate.began.load(ordering: .sequentiallyConsistent) == 0)
        #expect(MainDemo.callsSnapshot() == callsBefore)
    }

    @Test func aQueuedActionPastItsDeadlineNeverStarts() {
        let gate = Gate()
        let callsBefore = MainDemo.callsSnapshot()
        let blocker = MainThreadBlocker()
        let outcome = failure(invoke("main-demo", "whereAmI", "{}", gate: gate, within: .milliseconds(100)))
        blocker.unblock()
        usleep(50_000)
        #expect(outcome?.code == "INSPECTION_TIMEOUT")
        #expect(outcome?.stage == "queue")
        #expect(outcome?.targetExecutionMayContinue == false)
        #expect(MainDemo.callsSnapshot() == callsBefore)
    }

    @Test func aStartedActionPastItsDeadlineMayStillBeRunning() {
        let outcome = failure(
            invoke("main-demo", "hold", #"{"milliseconds":400}"#, within: .milliseconds(100)))
        #expect(outcome?.code == "INSPECTION_TIMEOUT")
        #expect(outcome?.stage == "target-execution")
        #expect(outcome?.targetExecutionMayContinue == true)
        usleep(400_000)
    }

    @Test func loopsObserveRevocationThroughTheirContext() {
        let gate = Gate()
        let pending = inBackground { invoke("main-demo", "spin", "{}", gate: gate) }
        usleep(100_000)
        gate.revoked.store(true, ordering: .sequentiallyConsistent)
        let outcome = failure(pending())
        #expect(outcome?.code == "INSPECTION_TIMEOUT")
        #expect(outcome?.stage == "target-execution")
        #expect(outcome?.targetExecutionMayContinue == false)
    }

    @Test func asyncActionsCompleteAndObserveTheirDeadline() throws {
        let paused = try #require(json(invoke("demo", "pause", #"{"milliseconds":10}"#)))
        let object = try JSONSerialization.jsonObject(with: Data(paused.utf8)) as? NSDictionary
        #expect(object == ["message": "paused", "excited": false])
        let late = failure(invoke("demo", "pause", #"{"milliseconds":300}"#, within: .milliseconds(50)))
        #expect(late?.code == "INSPECTION_TIMEOUT")
    }

    @Test func discoveryListsSchemasWithoutRunningActions() {
        let before = Calls.count
        let discovery = runtime.discovery()
        #expect(discovery.contains(#""extension":"demo","action":"greeting""#))
        #expect(Calls.count == before)
    }

    /// Measurement for the proof record, not a performance gate.
    @Test func outputValidationCost() throws {
        let count = 40_000
        let clock = ContinuousClock()
        let models = (0..<count).map {
            Measurement(
                identifier: "node-\($0)", ratio: Double($0) / 3, count: Int64($0),
                window: UInt32($0), note: $0.isMultiple(of: 2) ? "even" : nil)
        }
        var encoded = Data()
        let encodeOnly = clock.measure { encoded = try! JSONEncoder().encode(models) }
        var outcome: ReconOutcome?
        let invoked = clock.measure {
            outcome = invoke("demo", "bulk", #"{"count":\#(count)}"#, within: .seconds(10))
        }
        let result = try #require(outcome)
        let bytes = try #require(json(result)).utf8.count
        print("bulk \(count) records, \(bytes) bytes: encode \(encodeOnly), invoke+validate \(invoked)")
        #expect(bytes == encoded.count)
    }
}
}

extension MainDemo {
    nonisolated static func callsSnapshot() -> Int {
        DispatchQueue.main.sync { MainActor.assumeIsolated { calls } }
    }
}
