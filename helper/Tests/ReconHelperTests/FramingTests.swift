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

    @Test("Only a request of 1 to 65536 bytes is a frame")
    func requestLimits() {
        for length in UInt32(0)...65538 {
            #expect(ReconFrame.isValidRequest(length) == (length > 0 && length <= 65536))
        }
    }
}
