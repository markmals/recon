/// A member that is explicitly `null` when it has no value, rather than absent.
///
/// A synthesized Codable model omits a `nil` optional property; a payload that must publish
/// `null` in its place declares the member with this type instead.
public struct ReconNullable<Wrapped: Codable & ReconSchema & Sendable>: Codable, Sendable,
    ReconSchema
{
    public let value: Wrapped?

    public init(_ value: Wrapped?) { self.value = value }

    public init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        value = container.decodeNil() ? nil : try container.decode(Wrapped.self)
    }

    public func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        if let value {
            try container.encode(value)
        } else {
            try container.encodeNil()
        }
    }

    public static var reconSchema: ReconSchemaNode { .nullable(Wrapped.reconSchema) }
}

extension ReconNullable: Equatable where Wrapped: Equatable {}
