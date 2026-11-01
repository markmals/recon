import Testing

@testable import ReconHelper

@Suite("Build identity")
struct BuildIdentityTests {
    /// Only `helper:build` links the `__TEXT,__recon_build` section, so a test bundle has none.
    /// An image that was not built that way names no build and answers no request, rather than
    /// inventing an identity a client could match.
    @Test("An image without the build section names no build")
    func absentSection() {
        #expect(ReconHelperIdentity.build.isEmpty)
    }

    @Test("The helper speaks wire protocol 1")
    func protocolVersion() {
        #expect(ReconHelperIdentity.protocolVersion == 1)
    }
}
