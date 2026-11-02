import Darwin
import Foundation
import Testing

@testable import ReconHelper

@Suite("The handed endpoint")
struct EndpointTests {
    @Test("A handed stream socket is claimed with the path it was bound at")
    func claimsTheHandedSocket() throws {
        let directory = try privateDirectory()
        defer { rmdir(directory) }
        let port = try handedEndpoint(at: "\(directory)/s")

        let endpoint = try #require(ReconEndpoint.claim(port))
        defer { endpoint.discard() }

        #expect(endpoint.path == "\(directory)/s")
        #expect(endpoint.listen())
    }

    @Test("A port with no endpoint queued on it is refused without waiting")
    func refusesAnEmptyPort() throws {
        let port = try receiveRight()
        let started = Date.now

        #expect(ReconEndpoint.claim(port) == nil)
        #expect(Date.now.timeIntervalSince(started) < 1)
    }

    @Test("A handed socket that is not a stream is refused")
    func refusesADatagramSocket() throws {
        let directory = try privateDirectory()
        defer {
            unlink("\(directory)/s")
            rmdir(directory)
        }
        let port = try handedEndpoint(at: "\(directory)/s", type: SOCK_DGRAM)

        #expect(ReconEndpoint.claim(port) == nil)
    }

    @Test("Claiming releases the receive right the loader created")
    func releasesThePort() throws {
        let directory = try privateDirectory()
        defer { rmdir(directory) }
        let port = try handedEndpoint(at: "\(directory)/s")

        try #require(ReconEndpoint.claim(port)).discard()

        var type: mach_port_type_t = 0
        #expect(mach_port_type(mach_task_self_, port, &type) == KERN_INVALID_NAME)
    }
}
