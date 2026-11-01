import Testing

@testable import ReconHelper

@Suite("Session tokens")
struct TokenTests {
    @Test("A token is 64 lowercase hexadecimal characters")
    func shape() throws {
        let token = try #require(reconToken())
        #expect(token.count == 64)
        #expect(token.allSatisfy { $0.isHexDigit && !$0.isUppercase })
    }

    @Test("Two tokens differ")
    func distinct() throws {
        let first = try #require(reconToken())
        let second = try #require(reconToken())
        #expect(first != second)
    }
}
