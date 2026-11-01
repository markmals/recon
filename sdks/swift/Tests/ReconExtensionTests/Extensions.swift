import Foundation
import ReconExtension

@ReconExports("demo")
enum Demo {
    @ReconAction
    static func greeting(_ name: String) -> Greeting {
        Calls.record()
        return Greeting(message: "Hello, \(name)")
    }

    @ReconAction
    static func measure(
        id: String, ratio: Double, count: Int64, window: UInt32, note: String? = nil
    ) throws -> Measurement {
        Calls.record()
        return Measurement(identifier: id, ratio: ratio, count: count, window: window, note: note)
    }

    @ReconAction
    static func survey(points: [Point], limit: Int = 2) -> Survey {
        Survey(
            points: Array(points.prefix(limit)),
            completeness: points.count > limit ? .truncated : .complete,
            tags: ["points": points.count], maybe: ["kept", nil])
    }

    @ReconAction
    static func reference(handle: String? = nil) -> Reference { Reference(handle) }

    @ReconAction
    static func fail(kind: String) throws -> Greeting {
        Calls.record()
        switch kind {
        case "stale": throw ReconActionError.staleReference("node was deallocated")
        case "invalid": throw ReconActionError.invalidArgument("unknown window")
        case "cancelled": throw ReconActionError.cancelled
        case "staged": throw ReconActionError.invalidArgument("bad depth", stage: "bounds")
        case "unavailable": throw ReconActionError.unavailable("no handles", stage: "registry")
        default: throw Unexpected()
        }
    }

    @ReconAction
    static func unencodable() -> Measurement {
        Calls.record()
        return Measurement(identifier: "nan", ratio: .nan, count: 0, window: 1, note: nil)
    }

    @ReconAction
    static func misdeclared() -> Misdeclared { Misdeclared(value: "bare") }

    @ReconAction
    static func pause(milliseconds: Int, context: ReconContext) async throws -> Greeting {
        try await Task.sleep(for: .milliseconds(milliseconds))
        try context.checkCancellation()
        return Greeting(message: "paused", excited: false)
    }

    @ReconAction
    static func nothing() {}

    @ReconAction
    static func bulk(count: Int) -> [Measurement] {
        (0..<count).map {
            Measurement(
                identifier: "node-\($0)", ratio: Double($0) / 3, count: Int64($0),
                window: UInt32($0), note: $0.isMultiple(of: 2) ? "even" : nil)
        }
    }

    static func helper() -> String { "not exported" }
}

@ReconExports("main-demo")
@MainActor
enum MainDemo {
    static var calls = 0

    @ReconAction
    static func whereAmI() -> ThreadReport {
        calls += 1
        return ThreadReport(mainThread: Thread.isMainThread, calls: calls)
    }

    @ReconAction
    static func hold(milliseconds: Int) -> ThreadReport {
        calls += 1
        usleep(useconds_t(milliseconds * 1000))
        return ThreadReport(mainThread: Thread.isMainThread, calls: calls)
    }

    @ReconAction
    static func spin(context: ReconContext) throws -> ThreadReport {
        calls += 1
        while true {
            try context.checkCancellation()
            usleep(1000)
        }
    }

    @ReconAction
    nonisolated static func elsewhere() -> ThreadReport {
        ThreadReport(mainThread: Thread.isMainThread, calls: 0)
    }
}

@ReconExports("other")
enum Other {
    @ReconAction
    static func greeting(_ name: String) -> Greeting { Greeting(message: "Hi, \(name)") }
}
