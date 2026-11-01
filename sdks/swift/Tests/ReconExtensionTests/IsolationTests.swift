import Foundation
import Testing

extension ProposalSDK {
    @Suite
    struct IsolationAndLifecycle {
        @Test func mainActorActionsRunOnTheMainThreadAndOthersDoNot() {
            #expect(
                json(invoke("main-demo", "whereAmI", "{}"))?.contains(#""mainThread":true"#) == true)
            #expect(
                json(invoke("main-demo", "elsewhere", "{}"))?.contains(#""mainThread":false"#)
                    == true)
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
            #expect(outcome?.stage == "main-thread-queue")
            #expect(outcome?.targetExecutionMayContinue == nil)
            #expect(gate.began.load(ordering: .sequentiallyConsistent) == 0)
            #expect(MainDemo.callsSnapshot() == callsBefore)
        }

        @Test func aQueuedActionPastItsDeadlineNeverStarts() {
            let gate = Gate()
            let callsBefore = MainDemo.callsSnapshot()
            let blocker = MainThreadBlocker()
            let outcome = failure(
                invoke("main-demo", "whereAmI", "{}", gate: gate, within: .milliseconds(100)))
            blocker.unblock()
            usleep(50_000)
            #expect(outcome?.code == "INSPECTION_TIMEOUT")
            #expect(outcome?.stage == "main-thread-queue")
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
        }

        @Test func asyncActionsCompleteAndObserveTheirDeadline() throws {
            let paused = try #require(json(invoke("demo", "pause", #"{"milliseconds":10}"#)))
            let object = try JSONSerialization.jsonObject(with: Data(paused.utf8)) as? NSDictionary
            #expect(object == ["message": "paused", "excited": false])
            let late = failure(
                invoke("demo", "pause", #"{"milliseconds":300}"#, within: .milliseconds(50)))
            #expect(late?.code == "INSPECTION_TIMEOUT")
        }

        /// A nonisolated action is not queued on the target's main thread, and says so.
        @Test func aQueuedNonisolatedActionNamesItsOwnQueue() {
            let gate = Gate()
            gate.revoked.store(true, ordering: .sequentiallyConsistent)
            let outcome = failure(invoke("demo", "greeting", #"{"name":"Mark"}"#, gate: gate))
            #expect(outcome?.code == "STALE_SESSION")
            #expect(outcome?.stage == "queue")
        }

        @Test func resultsBeyondTheBoundAreRefused() {
            let outcome = failure(invoke("demo", "bulk", #"{"count":120000}"#, within: .seconds(20)))
            #expect(outcome?.code == "INSPECTION_UNAVAILABLE")
            #expect(outcome?.stage == "response-size")
        }
    }
}
