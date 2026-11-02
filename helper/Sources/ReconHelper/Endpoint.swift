import Darwin
import Foundation

/// The endpoint the loader hands this process: a Unix stream socket it bound in a directory only
/// the owner can reach, queued as a fileport in the one message on a receive right in this task.
///
/// The helper never binds. A sandboxed target may not create a socket outside its own container,
/// but it can listen on and accept from one it was given. Listening here, rather than in the
/// loader, also makes this process the peer every connecting client's audit token names.
struct ReconEndpoint {
    static let maximumPathBytes = MemoryLayout.size(ofValue: sockaddr_un().sun_path)

    let descriptor: Int32
    let path: String

    /// Takes the endpoint queued on `port` and releases the port, or nil when the port carries no
    /// endpoint, or carries anything but a bound Unix stream socket.
    static func claim(_ port: mach_port_name_t) -> ReconEndpoint? {
        guard port != mach_port_name_t(MACH_PORT_NULL) else { return nil }
        defer { mach_port_mod_refs(mach_task_self_, port, MACH_PORT_RIGHT_RECEIVE, -1) }
        guard let fileport = queuedFileport(on: port) else { return nil }
        let descriptor = fileport_makefd(fileport)
        mach_port_deallocate(mach_task_self_, fileport)
        guard descriptor >= 0 else { return nil }
        guard isStreamSocket(descriptor), let path = boundPath(of: descriptor) else {
            close(descriptor)
            return nil
        }
        return ReconEndpoint(descriptor: descriptor, path: path)
    }

    func listen() -> Bool {
        Darwin.listen(descriptor, 16) == 0
    }

    /// Gives the endpoint up. The target's sandbox may refuse the unlink; a socket nothing listens
    /// on refuses every client, and Recon removes a refused endpoint before it loads again.
    func discard() {
        close(descriptor)
        unlink(path)
    }

    /// A peer that disconnects mid-answer must fail the write, not the process.
    static func refuseSignalOnWrite(_ descriptor: Int32) {
        var enabled = Int32(1)
        setsockopt(
            descriptor, SOL_SOCKET, SO_NOSIGPIPE, &enabled, socklen_t(MemoryLayout<Int32>.size))
    }

    /// The loader queues the message before the helper runs, so receiving never waits. The
    /// message is read at its C offsets: a header, a body, then one port descriptor.
    private static func queuedFileport(on port: mach_port_name_t) -> mach_port_t? {
        let headerSize = MemoryLayout<mach_msg_header_t>.size
        let bodySize = MemoryLayout<mach_msg_body_t>.size
        let messageSize = headerSize + bodySize + MemoryLayout<mach_msg_port_descriptor_t>.size
        let capacity = messageSize + MemoryLayout<mach_msg_max_trailer_t>.size
        let buffer = UnsafeMutableRawPointer.allocate(
            byteCount: capacity, alignment: MemoryLayout<mach_msg_header_t>.alignment)
        defer { buffer.deallocate() }
        buffer.initializeMemory(as: UInt8.self, repeating: 0, count: capacity)
        let header = buffer.bindMemory(to: mach_msg_header_t.self, capacity: 1)
        let received = mach_msg(
            header, MACH_RCV_MSG | MACH_RCV_TIMEOUT, 0, mach_msg_size_t(capacity), port, 0,
            mach_port_name_t(MACH_PORT_NULL))
        guard received == KERN_SUCCESS else { return nil }
        let descriptors = buffer.load(fromByteOffset: headerSize, as: mach_msg_body_t.self)
        let endpoint = buffer.load(
            fromByteOffset: headerSize + bodySize, as: mach_msg_port_descriptor_t.self)
        guard header.pointee.msgh_bits & UInt32(MACH_MSGH_BITS_COMPLEX) != 0,
            header.pointee.msgh_size == messageSize, descriptors.msgh_descriptor_count == 1,
            endpoint.type == mach_msg_descriptor_type_t(MACH_MSG_PORT_DESCRIPTOR)
        else {
            mach_msg_destroy(header)
            return nil
        }
        return endpoint.name
    }

    private static func isStreamSocket(_ descriptor: Int32) -> Bool {
        var type = Int32(0)
        var length = socklen_t(MemoryLayout<Int32>.size)
        return getsockopt(descriptor, SOL_SOCKET, SO_TYPE, &type, &length) == 0
            && type == SOCK_STREAM
    }

    private static func boundPath(of descriptor: Int32) -> String? {
        var address = sockaddr_un()
        var length = socklen_t(MemoryLayout<sockaddr_un>.size)
        let named = withUnsafeMutablePointer(to: &address) { address in
            address.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                getsockname(descriptor, $0, &length)
            }
        }
        guard named == 0, address.sun_family == sa_family_t(AF_UNIX) else { return nil }
        let path = withUnsafeBytes(of: &address.sun_path) { field in
            String(cString: field.bindMemory(to: CChar.self).baseAddress!)
        }
        return path.isEmpty ? nil : path
    }
}
