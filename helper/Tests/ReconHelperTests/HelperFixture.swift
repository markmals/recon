import Darwin
import Foundation

@testable import ReconHelper

enum FixtureFailure: Error {
    case directory
    case start
    case handoff
    case connect
    case transport
    case malformedAnswer
}

/// A server listening on an endpoint handed to it the way the loader hands one over: bound in a
/// directory only this user can reach, and queued as a fileport on a receive right in this task.
///
/// The server has no shutdown: the loader starts one for the life of the target process. A
/// fixture therefore leaves its accept thread running and only removes what it put on disk.
final class HelperFixture {
    let path: String
    private let directory: String
    private let server: ReconHelperServer

    init(build: String = "fixture") throws {
        directory = try privateDirectory()
        path = "\(directory)/s"
        server = ReconHelperServer(build: build)
        guard server.start(try handedEndpoint(at: path)) == 0 else { throw FixtureFailure.start }
    }

    deinit {
        unlink(path)
        rmdir(directory)
    }

    /// A lifecycle request: the peer's claims about itself, the identity it expects the helper
    /// to have, and the access it wants.
    func request(_ operation: String, _ members: [String: Any] = [:]) -> [String: Any] {
        let incarnation = ReconProcessIncarnation.current
        return envelope(operation).merging([
            "expectedPid": incarnation.pid, "expectedPidversion": incarnation.pidversion,
            "expectedUid": incarnation.uid, "profile": "standard", "mode": "existing",
        ]) { _, kept in kept }.merging(members) { _, replacement in replacement }
    }

    /// A read carries the envelope, the session it reads for, and the action's own input: the
    /// runtime refuses a member the action does not publish.
    func read(
        _ operation: String, _ session: [String: Any], _ members: [String: Any] = [:]
    ) -> [String: Any] {
        envelope(operation).merging(session) { _, replacement in replacement }
            .merging(members) { _, replacement in replacement }
    }

    private func envelope(_ operation: String) -> [String: Any] {
        let incarnation = ReconProcessIncarnation.current
        return [
            "op": operation, "protocol": 2, "helperBuild": "fixture",
            "clientPid": incarnation.pid, "clientPidversion": incarnation.pidversion,
            "clientUid": incarnation.uid,
        ]
    }

    func exchange(_ request: [String: Any]) throws -> [String: Any] {
        let answer = try exchange(frame: try JSONSerialization.data(withJSONObject: request))
        guard let members = try JSONSerialization.jsonObject(with: answer) as? [String: Any] else {
            throw FixtureFailure.malformedAnswer
        }
        return members
    }

    func exchange(frame body: Data) throws -> Data {
        try exchange(header: UInt32(body.count), body: body)
    }

    /// A header with nothing behind it, for the frames the server refuses before reading one.
    func exchange(header length: UInt32, body: Data = Data()) throws -> Data {
        let descriptor = try connected()
        defer { close(descriptor) }
        try write(Data(ReconFrame.header(length)) + body, to: descriptor)
        return try answer(from: descriptor)
    }

    private func connected() throws -> Int32 {
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { throw FixtureFailure.connect }
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutableBytes(of: &address.sun_path) { field in
            _ = strlcpy(
                field.baseAddress?.assumingMemoryBound(to: CChar.self), path,
                ReconEndpoint.maximumPathBytes)
        }
        let joined = withUnsafePointer(to: &address) { address in
            address.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                connect(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        guard joined == 0 else {
            close(descriptor)
            throw FixtureFailure.connect
        }
        return descriptor
    }

    private func write(_ bytes: Data, to descriptor: Int32) throws {
        let written = bytes.withUnsafeBytes { Darwin.write(descriptor, $0.baseAddress, $0.count) }
        guard written == bytes.count else { throw FixtureFailure.transport }
    }

    private func answer(from descriptor: Int32) throws -> Data {
        var header = [UInt8](repeating: 0, count: 4)
        try read(into: &header, from: descriptor)
        var body = [UInt8](repeating: 0, count: Int(ReconFrame.length(header)))
        try read(into: &body, from: descriptor)
        return Data(body)
    }

    private func read(into bytes: inout [UInt8], from descriptor: Int32) throws {
        var offset = 0
        while offset < bytes.count {
            let count = bytes.count
            let moved = bytes.withUnsafeMutableBytes {
                Darwin.read(descriptor, $0.baseAddress! + offset, count - offset)
            }
            guard moved > 0 else { throw FixtureFailure.transport }
            offset += moved
        }
    }
}

/// A fresh directory only this user can reach, as Recon's descriptor root provides one.
func privateDirectory() throws -> String {
    var template = Array("/tmp/recon-helper-tests-XXXXXX".utf8CString)
    guard let made = mkdtemp(&template) else { throw FixtureFailure.directory }
    let directory = String(cString: made)
    guard chmod(directory, 0o700) == 0 else { throw FixtureFailure.directory }
    return directory
}

/// What the loader does before the helper runs: bind a socket of `type` at `path`, and queue it as
/// a fileport on a new receive right in this task, whose name it then passes to the entry point.
func handedEndpoint(at path: String, type: Int32 = SOCK_STREAM) throws -> mach_port_name_t {
    let descriptor = socket(AF_UNIX, type, 0)
    guard descriptor >= 0 else { throw FixtureFailure.handoff }
    defer { close(descriptor) }
    var address = sockaddr_un()
    address.sun_family = sa_family_t(AF_UNIX)
    withUnsafeMutableBytes(of: &address.sun_path) { field in
        _ = strlcpy(
            field.baseAddress?.assumingMemoryBound(to: CChar.self), path,
            ReconEndpoint.maximumPathBytes)
    }
    let bound = withUnsafePointer(to: &address) { address in
        address.withMemoryRebound(to: sockaddr.self, capacity: 1) {
            bind(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
        }
    }
    guard bound == 0, chmod(path, 0o600) == 0 else { throw FixtureFailure.handoff }
    var fileport: fileport_t = 0
    guard fileport_makeport(descriptor, &fileport) == 0 else { throw FixtureFailure.handoff }
    let port = try receiveRight()
    try queue(fileport, on: port)
    return port
}

/// A receive right in this task with nothing queued on it.
func receiveRight() throws -> mach_port_name_t {
    var port: mach_port_name_t = 0
    guard mach_port_allocate(mach_task_self_, MACH_PORT_RIGHT_RECEIVE, &port) == KERN_SUCCESS
    else { throw FixtureFailure.handoff }
    return port
}

/// Queues one message carrying `fileport` on `port`, moving this task's send right into it.
func queue(_ fileport: mach_port_t, on port: mach_port_name_t) throws {
    struct Handoff {
        var header = mach_msg_header_t()
        var body = mach_msg_body_t()
        var endpoint = mach_msg_port_descriptor_t()
    }
    var message = Handoff()
    message.header.msgh_bits = UInt32(MACH_MSG_TYPE_MAKE_SEND) | UInt32(MACH_MSGH_BITS_COMPLEX)
    message.header.msgh_size = mach_msg_size_t(MemoryLayout<Handoff>.size)
    message.header.msgh_remote_port = port
    message.body.msgh_descriptor_count = 1
    message.endpoint.name = fileport
    message.endpoint.disposition = mach_msg_type_name_t(MACH_MSG_TYPE_MOVE_SEND)
    message.endpoint.type = mach_msg_descriptor_type_t(MACH_MSG_PORT_DESCRIPTOR)
    let sent = withUnsafeMutablePointer(to: &message) { message in
        message.withMemoryRebound(to: mach_msg_header_t.self, capacity: 1) {
            mach_msg(
                $0, MACH_SEND_MSG, mach_msg_size_t(MemoryLayout<Handoff>.size), 0,
                mach_port_name_t(MACH_PORT_NULL), 0, mach_port_name_t(MACH_PORT_NULL))
        }
    }
    guard sent == KERN_SUCCESS else { throw FixtureFailure.handoff }
}

/// What a refused answer says, as every test that reads one asks it.
extension Dictionary where Key == String, Value == Any {
    var errorCode: String? { error?["code"] as? String }
    var errorStage: String? { error?["stage"] as? String }
    var errorMessage: String? { error?["message"] as? String }

    private var error: [String: Any]? { self["error"] as? [String: Any] }
}
