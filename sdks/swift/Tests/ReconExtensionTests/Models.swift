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

/// A value whose published shape is a union, as the AppKit observations' statuses are.
struct Reference: Codable, Equatable, ReconSchema {
    let handle: String?

    init(_ handle: String?) { self.handle = handle }

    init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        handle = try? container.decode(String.self)
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        if let handle {
            try container.encode(handle)
        } else {
            try container.encode(["status": "absent"])
        }
    }

    static var reconSchema: ReconSchemaNode {
        .oneOf([
            .string,
            .object([ReconSchemaProperty("status", .string(enumeration: ["absent"]), required: true)]),
        ])
    }
}

struct Unexpected: Error {}

enum Calls {
    private static let invocations = Mutex(0)

    static var count: Int { invocations.withLock { $0 } }
    static func record() { invocations.withLock { $0 += 1 } }
}
