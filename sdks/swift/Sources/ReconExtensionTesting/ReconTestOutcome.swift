import Foundation

/// What one invocation answered: the encoded result, or the failure the host would publish.
public enum ReconTestOutcome: Sendable, Equatable {
    case success(Data)
    case failure(code: String, stage: String, message: String)

    /// The failure's classification, or nil for a success.
    public var failure: (code: String, stage: String, message: String)? {
        guard case .failure(let code, let stage, let message) = self else { return nil }
        return (code, stage, message)
    }

    /// The encoded result decoded back into a Swift value.
    public func decode<T: Decodable>(_ type: T.Type) throws -> T {
        switch self {
        case .success(let result): try JSONDecoder().decode(type, from: result)
        case .failure(let code, let stage, let message):
            throw ReconUnexpectedOutcome(code: code, stage: stage, message: message)
        }
    }
}

/// A result was expected and the action failed instead.
public struct ReconUnexpectedOutcome: Error, Equatable, CustomStringConvertible {
    public let code: String
    public let stage: String
    public let message: String

    public var description: String { "\(code) at \(stage): \(message)" }
}

/// An action encoded a result its published schema does not describe.
///
/// A macro cannot see a `CodingKeys` or a custom codec declared in another file, so the runtime
/// cannot police results without re-parsing every one of them. `ReconTestHost` checks each
/// result instead, and this is how a schema that disagrees with its encoding is caught.
public struct ReconSchemaMismatch: Error, Equatable, CustomStringConvertible {
    public let extensionIdentifier: String
    public let action: String
    public let violation: String

    public var description: String {
        "the result of '\(action)' in extension '\(extensionIdentifier)' does not match its "
            + "published schema: \(violation)"
    }
}
