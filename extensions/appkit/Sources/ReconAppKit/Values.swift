import ReconExtension

/// Why a published value has no value, in the vocabulary proposal.0005 established.
@ReconModel
enum StatusName: String, Codable, Sendable {
    case value
    case absent
    case unavailable
    case failed
    case unsupported
    case ambiguous
    case nonFinite = "non-finite"
    case noIntrinsicMetric = "no-intrinsic-metric"
}

extension ReconSchemaNode {
    /// `{"status": …}`: the object a value takes when it has nothing to report.
    static func status(_ names: StatusName...) -> ReconSchemaNode {
        .object([
            ReconSchemaProperty(
                "status", .string(enumeration: names.map(\.rawValue)), required: true)
        ])
    }
}

/// A status object on its own, as a facet that an object does not support answers.
struct Status: Codable, Equatable, ReconSchema {
    let status: StatusName

    init(_ status: StatusName) { self.status = status }

    static var reconSchema: ReconSchemaNode { .status(.unsupported, .absent, .unavailable) }
}

/// A native handle, or the status that explains why there is none. The handle is the whole
/// value: a reference is a string on the wire, not an object wrapping one.
struct Reference: Codable, Equatable, ReconSchema {
    let handle: String?
    let absence: StatusName

    init(_ handle: String?, absence: StatusName = .absent) {
        self.handle = handle
        self.absence = absence
    }

    init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let handle = try? container.decode(String.self) {
            self.handle = handle
            absence = .absent
        } else {
            handle = nil
            absence = try container.decode(Status.self).status
        }
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        if let handle {
            try container.encode(handle)
        } else {
            try container.encode(Status(absence))
        }
    }

    static var reconSchema: ReconSchemaNode { .oneOf([.string, .status(.absent, .unavailable)]) }
}

/// A number that is the value itself, with a non-finite one reported as the absence of a number.
struct Scalar: Codable, Equatable, ReconSchema {
    let number: Double?

    init(_ number: Double) { self.number = number.isFinite ? number : nil }

    init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        number = try? container.decode(Double.self)
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        if let number {
            try container.encode(number)
        } else {
            try container.encode(Status(.nonFinite))
        }
    }

    static var reconSchema: ReconSchemaNode { .oneOf([.number, .status(.nonFinite)]) }
}

/// A font weight, which a descriptor may not carry at all.
struct OptionalScalar: Codable, Equatable, ReconSchema {
    let number: Double?
    let absence: StatusName

    init(_ number: Double?) {
        self.number = number.flatMap { $0.isFinite ? $0 : nil }
        absence = number == nil ? .absent : .nonFinite
    }

    init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let number = try? container.decode(Double.self) {
            self.number = number
            absence = .absent
        } else {
            number = nil
            absence = try container.decode(Status.self).status
        }
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        if let number {
            try container.encode(number)
        } else {
            try container.encode(Status(absence))
        }
    }

    static var reconSchema: ReconSchemaNode {
        .oneOf([.number, .status(.nonFinite, .absent)])
    }
}

/// A number reported as a value, so that a non-finite one is a status rather than a number.
@ReconModel
struct NumberValue: Codable, Equatable {
    let status: StatusName
    let value: Double?

    init(_ number: Double) {
        status = number.isFinite ? .value : .nonFinite
        value = number.isFinite ? number : nil
    }
}

/// A string an object either carries or does not.
@ReconModel
struct TextValue: Codable, Equatable {
    let status: StatusName
    let value: String?

    init(_ text: String?) {
        status = text == nil ? .absent : .value
        value = text
    }
}

/// An integer an object either carries or does not.
@ReconModel
struct IntegerValue: Codable, Equatable {
    let status: StatusName
    let value: Int?

    init(_ number: Int?) {
        status = number == nil ? .absent : .value
        value = number
    }
}

/// A bare string, or the status that explains why there is none.
struct TextOrStatus: Codable, Equatable, ReconSchema {
    let text: String?

    init(_ text: String?) { self.text = text }

    init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        text = try? container.decode(String.self)
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        if let text {
            try container.encode(text)
        } else {
            try container.encode(Status(.absent))
        }
    }

    static var reconSchema: ReconSchemaNode { .oneOf([.string, .status(.absent)]) }
}

/// A display identifier, which the screen reports as a number or not at all.
struct DisplayIdentifier: Codable, Equatable, ReconSchema {
    let number: Int?

    init(_ number: Int?) { self.number = number }

    init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        number = try? container.decode(Int.self)
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        if let number {
            try container.encode(number)
        } else {
            try container.encode(Status(.absent))
        }
    }

    static var reconSchema: ReconSchemaNode { .oneOf([.of(Int.self), .status(.absent)]) }
}
