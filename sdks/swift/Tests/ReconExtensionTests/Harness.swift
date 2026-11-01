import Foundation
import ReconExtension
import ReconExtensionRuntime
import Synchronization
import Testing

/// One session's access, as the helper's server supplies it.
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
    _ extensionIdentifier: String, _ name: String, _ payload: String, gate: Gate = Gate(),
    within duration: Duration = .seconds(2)
) -> ReconOutcome {
    runtime.invoke(
        extensionIdentifier, name, payload: Data(payload.utf8), access: gate.access,
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

/// Runs an invocation off the test's own thread, so the test can revoke access or release the
/// main thread while it is still queued.
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

extension MainDemo {
    nonisolated static func callsSnapshot() -> Int {
        DispatchQueue.main.sync { MainActor.assumeIsolated { calls } }
    }
}

/// One serialized parent: the suites share invocation counters and the main thread.
@Suite(.serialized)
struct ProposalSDK {}
