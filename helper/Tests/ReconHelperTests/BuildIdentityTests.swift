import Darwin
import Foundation
import ReconExtensionRuntime
import Testing

@testable import ReconHelper

@Suite("Build identity")
struct BuildIdentityTests {
    /// Only `helper:build` links the `__TEXT,__recon_build` section, so a test bundle has none.
    /// An image that was not built that way does not serve: naming no build, it would answer
    /// every client that names none either.
    @Test("An image without the build section names no build")
    func absentSection() {
        #expect(ReconHelperIdentity.build.isEmpty)
    }

    @Test("A server with no build identity refuses to start and gives its endpoint up")
    func refusesToStart() throws {
        let directory = try privateDirectory()
        defer { rmdir(directory) }
        let port = try handedEndpoint(at: "\(directory)/s")
        #expect(ReconHelperServer(build: "").start(port) == -1)
        #expect(!FileManager.default.fileExists(atPath: "\(directory)/s"))
    }

    /// A request that names no build is refused however the helper was built, so an image that
    /// lost its section cannot be talked to by matching its emptiness.
    @Test("A request naming no build is refused")
    func refusesAnEmptyBuild() throws {
        let fixture = try HelperFixture()
        let named = try fixture.exchange(fixture.request("reconcile", ["helperBuild": ""]))
        #expect(errorCode(named) == "INSPECTION_PROTOCOL_MISMATCH")

        let gate = ReconRouter(
            build: "", lifecycle: ReconAccessLifecycle(incarnation: .current, build: ""),
            runtime: try ReconRuntime(assembling: assembledExtensions))
        let refused = gate.answer(
            ["op": "reconcile", "protocol": 2, "helperBuild": ""], startedAt: .now)
        let members = try JSONSerialization.jsonObject(with: try #require(refused))
        #expect(errorCode(members as? [String: Any] ?? [:]) == "INSPECTION_PROTOCOL_MISMATCH")
    }

    private func errorCode(_ answer: [String: Any]) -> String? {
        (answer["error"] as? [String: Any])?["code"] as? String
    }

    @Test("The helper speaks wire protocol 2, which an earlier resident does not")
    func protocolVersion() {
        #expect(ReconHelperIdentity.protocolVersion == 2)
    }
}
