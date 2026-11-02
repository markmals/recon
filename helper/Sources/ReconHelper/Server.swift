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

    /// Starts serving on the endpoint the loader queued on `port`, and answers 0 once it is
    /// accepting. A process serves one endpoint: a second start is refused, and its endpoint given
    /// up, rather than replacing the first.
    func start(_ port: mach_port_name_t) -> Int32 {
        guard let endpoint = ReconEndpoint.claim(port) else { return -1 }
        // An image whose build identity was never linked in cannot be matched to a client, so
        // it does not serve: answering would mean answering whoever names no build either.
        guard let runtime, !build.isEmpty,
            started.compareExchange(
                expected: false, desired: true, ordering: .sequentiallyConsistent
            ).exchanged
        else {
            endpoint.discard()
            return -1
        }
        let incarnation = ReconProcessIncarnation.current
        guard incarnation.pidversion != 0, endpoint.listen() else {
            endpoint.discard()
            return startupFailed()
        }
        let router = ReconRouter(
            build: build,
            lifecycle: ReconAccessLifecycle(incarnation: incarnation, build: build),
            runtime: runtime)
        Thread.detachNewThread { self.accept(on: endpoint, router) }
        return 0
    }

    /// Accepting ends only when the listener itself fails, and the endpoint it owns goes with
    /// it: nothing else can serve this path afterwards.
    private func accept(on endpoint: ReconEndpoint, _ router: ReconRouter) {
        while true {
            let client = Darwin.accept(endpoint.descriptor, nil, nil)
            guard client >= 0 else {
                if errno == EINTR { continue }
                break
            }
            // This loop runs for the life of the inspected process, so each turn drains what it
            // autoreleased rather than growing the target's footprint until it exits.
            autoreleasepool {
                ReconEndpoint.refuseSignalOnWrite(client)
                Thread.detachNewThread { ReconConnection(descriptor: client).serve(router) }
            }
        }
        endpoint.discard()
    }

    private func startupFailed() -> Int32 {
        started.store(false, ordering: .sequentiallyConsistent)
        return -1
    }
}
