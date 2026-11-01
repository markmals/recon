import Darwin
import Foundation
import ReconExtensionRuntime
import Synchronization

/// The target-resident server: one endpoint, one thread per client, one assembled action set.
final class ReconHelperServer: Sendable {
    /// Assembly failure is a startup failure, not a crash: the server refuses to start rather
    /// than serving an extension set it could not build.
    private let runtime = try? ReconRuntime(assembling: assembledExtensions)
    private let started = Atomic<Bool>(false)
    private let build: String

    init(build: String = ReconHelperIdentity.build) {
        self.build = build
    }

    /// Starts serving on `path`, and answers 0 once the endpoint is accepting. A process serves
    /// one endpoint: a second start is refused rather than replacing the first.
    func start(_ path: String) -> Int32 {
        guard let runtime, !path.isEmpty, path.utf8.count < ReconEndpoint.maximumPathBytes else {
            return -1
        }
        guard
            started.compareExchange(
                expected: false, desired: true, ordering: .sequentiallyConsistent
            ).exchanged
        else { return -1 }
        let incarnation = ReconProcessIncarnation.current
        guard incarnation.pidversion != 0, let listener = ReconEndpoint.listener(at: path) else {
            return startupFailed()
        }
        let router = ReconRouter(
            build: build,
            lifecycle: ReconAccessLifecycle(incarnation: incarnation, build: build),
            runtime: runtime)
        Thread.detachNewThread { self.accept(on: listener, at: path, router) }
        return 0
    }

    /// Accepting ends only when the listener itself fails, and the endpoint it owns goes with
    /// it: nothing else can serve this path afterwards.
    private func accept(on listener: Int32, at path: String, _ router: ReconRouter) {
        while true {
            let client = Darwin.accept(listener, nil, nil)
            guard client >= 0 else {
                if errno == EINTR { continue }
                break
            }
            ReconEndpoint.refuseSignalOnWrite(client)
            Thread.detachNewThread { ReconConnection(descriptor: client).serve(router) }
        }
        close(listener)
        unlink(path)
    }

    private func startupFailed() -> Int32 {
        started.store(false, ordering: .sequentiallyConsistent)
        return -1
    }
}
