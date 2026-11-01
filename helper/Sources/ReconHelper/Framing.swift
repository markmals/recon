import Foundation

/// Length-prefixed framing: four big-endian bytes, then that many bytes of JSON.
enum ReconFrame {
    /// A request is read into one bounded allocation, so its length is bounded too.
    static let maximumRequestBytes: UInt32 = 65536
    static let maximumResponseBytes = 8 * 1024 * 1024

    static func header(_ length: UInt32) -> [UInt8] {
        [
            UInt8(truncatingIfNeeded: length >> 24), UInt8(truncatingIfNeeded: length >> 16),
            UInt8(truncatingIfNeeded: length >> 8), UInt8(truncatingIfNeeded: length),
        ]
    }

    static func length(_ header: [UInt8]) -> UInt32 {
        UInt32(header[0]) << 24 | UInt32(header[1]) << 16 | UInt32(header[2]) << 8
            | UInt32(header[3])
    }

    /// An answer that could not be built, or that does not fit the frame, is replaced rather
    /// than truncated: a client must never decode half an observation.
    static func bounded(_ response: Data?) -> Data {
        guard let response, response.count <= maximumResponseBytes else {
            let refusal = ReconEnvelope.error(
                "INSPECTION_UNAVAILABLE", "response-size", "Native response exceeds 8 MiB")
            return ReconEnvelope.serialized(refusal) ?? Data()
        }
        return response
    }

    static func isValidRequest(_ length: UInt32) -> Bool {
        length > 0 && length <= maximumRequestBytes
    }
}
