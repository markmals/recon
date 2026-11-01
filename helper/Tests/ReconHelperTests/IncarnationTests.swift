import Darwin
import Testing

@testable import ReconHelper

@Suite("Process incarnations")
struct IncarnationTests {
    @Test("This process reports its own pid, owner and generation")
    func current() {
        let incarnation = ReconProcessIncarnation.current
        #expect(incarnation.pid == getpid())
        #expect(incarnation.uid == geteuid())
        #expect(incarnation.pidversion != 0)
    }

    @Test("A connected peer's audit token names the process behind it")
    func peer() throws {
        var pair: [Int32] = [-1, -1]
        #expect(socketpair(AF_UNIX, SOCK_STREAM, 0, &pair) == 0)
        defer {
            close(pair[0])
            close(pair[1])
        }
        let peer = try #require(ReconProcessIncarnation.peer(of: pair[0]))
        #expect(peer.pid == getpid())
        #expect(peer.uid == geteuid())
        #expect(peer.pidversion == ReconProcessIncarnation.current.pidversion)
    }

    @Test("An unconnected descriptor has no peer")
    func withoutAPeer() {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        defer { close(descriptor) }
        #expect(ReconProcessIncarnation.peer(of: descriptor) == nil)
    }
}
