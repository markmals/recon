import Foundation
import ReconExtension
import Synchronization

@ReconModel
struct Greeting: Codable, Equatable {
    let message: String
    var excited: Bool?
}

@ReconModel
struct Measurement: Codable {
    let identifier: String
    let ratio: Double
    let count: Int64
    let window: UInt32
    let note: String?
    var ignored: Int = 0

    enum CodingKeys: String, CodingKey {
        case identifier = "id"
        case ratio, count
        case window = "windowId"
        case note
    }
}

@ReconModel
enum Completeness: String, Codable {
    case complete
    case truncated = "truncated-by-limit"
}

/// A custom codec: a point encodes as `[x, y]`, so its schema is stated, not derived.
struct Point: Codable, Equatable {
    let x: Double
    let y: Double

    init(x: Double, y: Double) {
        self.x = x
        self.y = y
    }

    init(from decoder: any Decoder) throws {
        var container = try decoder.unkeyedContainer()
        x = try container.decode(Double.self)
        y = try container.decode(Double.self)
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.unkeyedContainer()
        try container.encode(x)
        try container.encode(y)
    }
}

extension Point: ReconSchema {
    static var reconSchema: ReconSchemaNode { .array(.number) }
}

/// A custom codec whose stated schema is wrong: it encodes a bare string.
struct Misdeclared: Codable, ReconSchema {
    let value: String

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(value)
    }

    static var reconSchema: ReconSchemaNode {
        .object([ReconSchemaProperty("value", .string, required: true)])
    }
}

@ReconModel
struct Survey: Codable {
    let points: [Point]
    let completeness: Completeness
    let tags: [String: Int]
    let maybe: [String?]
}

@ReconModel
struct ThreadReport: Codable {
    let mainThread: Bool
    let calls: Int
}

struct Unexpected: Error {}

enum Calls {
    static let demo = Mutex(0)
    static var count: Int { demo.withLock { $0 } }
    static func record() { demo.withLock { $0 += 1 } }
}

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
    static func fail(kind: String) throws -> Greeting {
        Calls.record()
        switch kind {
        case "stale": throw ReconActionError.staleReference("node was deallocated")
        case "invalid": throw ReconActionError.invalidArgument("unknown window")
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
                identifier: "node-\($0)", ratio: Double($0) / 3, count: Int64($0), window: UInt32($0),
                note: $0.isMultiple(of: 2) ? "even" : nil)
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
