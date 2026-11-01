import Foundation
import Testing

@testable import ReconHelper

@Suite("Length-prefixed framing")
struct FramingTests {
    @Test("A length survives the header it is written to")
    func headerRoundTrip() {
        for length in UInt32(0)...65538 {
            #expect(ReconFrame.length(ReconFrame.header(length)) == length)
        }
    }

    @Test("The header is four bytes, most significant first")
    func byteOrder() {
        #expect(ReconFrame.header(0x0102_0304) == [1, 2, 3, 4])
        #expect(ReconFrame.length([0xFF, 0xFF, 0xFF, 0xFF]) == UInt32.max)
    }

    /// A client must never decode half an observation, so an answer that does not fit the
    /// frame is replaced by one that says so rather than truncated.
    @Test("An answer beyond 8 MiB is replaced, not truncated")
    func responseLimit() throws {
        let fits = ReconFrame.bounded(Data(count: ReconFrame.maximumResponseBytes))
        #expect(fits.count == ReconFrame.maximumResponseBytes)

        for answer in [Data(count: ReconFrame.maximumResponseBytes + 1), nil] {
            let refusal = ReconFrame.bounded(answer)
            let members = try JSONSerialization.jsonObject(with: refusal) as? [String: Any]
            let error = try #require(members?["error"] as? [String: Any])
            #expect(error["code"] as? String == "INSPECTION_UNAVAILABLE")
            #expect(error["stage"] as? String == "response-size")
            #expect(refusal.count < ReconFrame.maximumResponseBytes)
        }
    }

    @Test("Only a request of 1 to 65536 bytes is a frame")
    func requestLimits() {
        for length in UInt32(0)...65538 {
            #expect(ReconFrame.isValidRequest(length) == (length > 0 && length <= 65536))
        }
    }
}
