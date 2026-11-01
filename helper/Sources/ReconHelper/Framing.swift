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

    static func isValidRequest(_ length: UInt32) -> Bool {
        length > 0 && length <= maximumRequestBytes
    }
}
