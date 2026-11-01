import Foundation

/// Which phase of an invocation failed. Generated adapters keep the phases apart so the runtime
/// can report invalid input, an action's own error, and an unencodable result truthfully.
public enum ReconInvocationFailure: Error {
    case invalidInput(String)
    case action(any Error)
    case encoding(String)
}

/// The codec boundary generated adapters call. Settings live here, not in extension code.
public enum ReconCodec {
    public static func decode<T: Decodable>(_ type: T.Type, from payload: Data)
        throws(ReconInvocationFailure) -> T
    {
        do {
            return try JSONDecoder().decode(type, from: payload)
        } catch {
            throw .invalidInput(String(describing: error).prefix(256).description)
        }
    }

    public static func perform<R>(_ body: () throws -> R) throws(ReconInvocationFailure) -> R {
        do { return try body() } catch { throw .action(error) }
    }

    public static func performAsync<R>(_ body: () async throws -> R)
        async throws(ReconInvocationFailure) -> R
    {
        do { return try await body() } catch { throw .action(error) }
    }

    public static func encode<R: Encodable>(_ result: R) throws(ReconInvocationFailure) -> Data {
        do {
            return try JSONEncoder().encode(result)
        } catch {
            throw .encoding(String(describing: error).prefix(256).description)
        }
    }

    public static func encodeEmpty() -> Data { Data("{}".utf8) }
}
