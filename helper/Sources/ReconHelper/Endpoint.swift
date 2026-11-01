import Darwin
import Foundation

/// The endpoint the server listens on: a socket only its owner can reach, in a directory only
/// its owner can reach.
enum ReconEndpoint {
    static let maximumPathBytes = MemoryLayout.size(ofValue: sockaddr_un().sun_path)

    /// A bound, listening descriptor, or nil where the path, its directory, or the kernel
    /// refuses the endpoint. A failure after the bind leaves nothing behind on disk.
    static func listener(at path: String) -> Int32? {
        guard isPrivateDirectory(containing: path), !exists(path) else { return nil }
        let descriptor = socket(AF_UNIX, SOCK_STREAM, 0)
        guard descriptor >= 0 else { return nil }
        refuseSignalOnWrite(descriptor)
        guard bound(descriptor, to: path) else {
            close(descriptor)
            return nil
        }
        guard isOwnedSocket(path), chmod(path, 0o600) == 0, listen(descriptor, 16) == 0 else {
            close(descriptor)
            unlink(path)
            return nil
        }
        return descriptor
    }

    /// A peer that disconnects mid-answer must fail the write, not the process.
    static func refuseSignalOnWrite(_ descriptor: Int32) {
        var enabled = Int32(1)
        setsockopt(
            descriptor, SOL_SOCKET, SO_NOSIGPIPE, &enabled, socklen_t(MemoryLayout<Int32>.size))
    }

    private static func bound(_ descriptor: Int32, to path: String) -> Bool {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        withUnsafeMutableBytes(of: &address.sun_path) { field in
            _ = strlcpy(
                field.baseAddress?.assumingMemoryBound(to: CChar.self), path, maximumPathBytes)
        }
        return withUnsafePointer(to: &address) { address in
            address.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(descriptor, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) == 0
            }
        }
    }

    /// Nothing protects the endpoint before it exists except the directory holding it.
    private static func isPrivateDirectory(containing path: String) -> Bool {
        var directory = stat()
        guard lstat((path as NSString).deletingLastPathComponent, &directory) == 0 else {
            return false
        }
        return directory.st_mode & S_IFMT == S_IFDIR && directory.st_uid == geteuid()
            && directory.st_mode & 0o777 == 0o700
    }

    private static func isOwnedSocket(_ path: String) -> Bool {
        var endpoint = stat()
        guard lstat(path, &endpoint) == 0 else { return false }
        return endpoint.st_mode & S_IFMT == S_IFSOCK && endpoint.st_uid == geteuid()
    }

    private static func exists(_ path: String) -> Bool {
        var endpoint = stat()
        return lstat(path, &endpoint) == 0
    }
}
