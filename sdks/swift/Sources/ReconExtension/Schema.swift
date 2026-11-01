import Foundation

/// The published shape of one serialized payload: a strict JSON Schema subset.
///
/// Object properties that are not `required` may be absent but never `null`; `nullable` is the only
/// way a schema admits `null`. That keeps one schema exact for both Decodable inputs (validated
/// before decoding) and Encodable results (synthesized `encodeIfPresent` omits `nil`).
public indirect enum ReconSchemaNode: Sendable, Equatable {
    case string(enumeration: [String]?)
    case boolean
    case integer(minimum: Int128, maximum: Int128)
    case number
    case array(ReconSchemaNode)
    case dictionary(ReconSchemaNode)
    case object([ReconSchemaProperty])
    case nullable(ReconSchemaNode)

    public static let string = ReconSchemaNode.string(enumeration: nil)
}

public struct ReconSchemaProperty: Sendable, Equatable {
    public let name: String
    public let schema: ReconSchemaNode
    public let required: Bool

    public init(_ name: String, _ schema: ReconSchemaNode, required: Bool) {
        self.name = name
        self.schema = schema
        self.required = required
    }
}

/// A type whose serialized representation has a published schema.
///
/// `@ReconModel` generates this for synthesized Codable models. A type with a custom codec conforms
/// explicitly and states the shape its `encode(to:)`/`init(from:)` actually produce.
public protocol ReconSchema {
    static var reconSchema: ReconSchemaNode { get }
}

extension ReconSchemaNode {
    /// Resolves a type's schema in generated code, so a missing conformance names the type.
    public static func of<T: ReconSchema>(_ type: T.Type) -> ReconSchemaNode { T.reconSchema }
}

extension String: ReconSchema { public static var reconSchema: ReconSchemaNode { .string } }
extension Bool: ReconSchema { public static var reconSchema: ReconSchemaNode { .boolean } }
extension Double: ReconSchema { public static var reconSchema: ReconSchemaNode { .number } }
extension Float: ReconSchema { public static var reconSchema: ReconSchemaNode { .number } }

extension FixedWidthInteger {
    static var reconIntegerSchema: ReconSchemaNode {
        .integer(minimum: Int128(Self.min), maximum: Int128(Self.max))
    }
}

extension Int: ReconSchema { public static var reconSchema: ReconSchemaNode { reconIntegerSchema } }
extension Int8: ReconSchema { public static var reconSchema: ReconSchemaNode { reconIntegerSchema } }
extension Int16: ReconSchema { public static var reconSchema: ReconSchemaNode { reconIntegerSchema } }
extension Int32: ReconSchema { public static var reconSchema: ReconSchemaNode { reconIntegerSchema } }
extension Int64: ReconSchema { public static var reconSchema: ReconSchemaNode { reconIntegerSchema } }
extension UInt: ReconSchema { public static var reconSchema: ReconSchemaNode { reconIntegerSchema } }
extension UInt8: ReconSchema { public static var reconSchema: ReconSchemaNode { reconIntegerSchema } }
extension UInt16: ReconSchema { public static var reconSchema: ReconSchemaNode { reconIntegerSchema } }
extension UInt32: ReconSchema { public static var reconSchema: ReconSchemaNode { reconIntegerSchema } }
extension UInt64: ReconSchema { public static var reconSchema: ReconSchemaNode { reconIntegerSchema } }

extension Array: ReconSchema where Element: ReconSchema {
    public static var reconSchema: ReconSchemaNode { .array(Element.reconSchema) }
}

extension Dictionary: ReconSchema where Key == String, Value: ReconSchema {
    public static var reconSchema: ReconSchemaNode { .dictionary(Value.reconSchema) }
}

/// A bare optional encodes `nil` as `null` (array elements, dictionary values, whole payloads).
/// Optional stored properties are handled by `@ReconModel` instead: absent, never `null`.
extension Optional: ReconSchema where Wrapped: ReconSchema {
    public static var reconSchema: ReconSchemaNode { .nullable(Wrapped.reconSchema) }
}

extension ReconSchemaNode {
    /// JSON Schema text, as the command registry would publish it.
    public var jsonSchema: String {
        switch self {
        case .string(let enumeration?):
            return "{\"enum\":[\(enumeration.map(quoted).joined(separator: ","))]}"
        case .string(nil):
            return "{\"type\":\"string\"}"
        case .boolean:
            return "{\"type\":\"boolean\"}"
        case .integer(let minimum, let maximum):
            return "{\"type\":\"integer\",\"minimum\":\(minimum),\"maximum\":\(maximum)}"
        case .number:
            return "{\"type\":\"number\"}"
        case .array(let element):
            return "{\"type\":\"array\",\"items\":\(element.jsonSchema)}"
        case .dictionary(let value):
            return "{\"type\":\"object\",\"additionalProperties\":\(value.jsonSchema)}"
        case .object(let properties):
            let members = properties.map { "\(quoted($0.name)):\($0.schema.jsonSchema)" }
            let required = properties.filter(\.required).map { quoted($0.name) }
            return "{\"type\":\"object\",\"properties\":{\(members.joined(separator: ","))},"
                + "\"required\":[\(required.joined(separator: ","))],\"additionalProperties\":false}"
        case .nullable(let wrapped):
            return "{\"anyOf\":[\(wrapped.jsonSchema),{\"type\":\"null\"}]}"
        }
    }
}

private func quoted(_ text: String) -> String {
    var escaped = "\""
    for scalar in text.unicodeScalars {
        switch scalar {
        case "\"": escaped += "\\\""
        case "\\": escaped += "\\\\"
        case _ where scalar.value < 0x20: escaped += String(format: "\\u%04x", scalar.value)
        default: escaped.unicodeScalars.append(scalar)
        }
    }
    return escaped + "\""
}
