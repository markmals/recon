import Darwin
import ReconNativeSupport

/// A process incarnation as the kernel reports it: the identity a peer cannot forge, and the
/// one the helper publishes about itself.
struct ReconProcessIncarnation: Equatable, Sendable {
    let pid: Int32
    let uid: UInt32
    /// The audit token's generation count, which separates this process from a later one that
    /// reuses its pid.
    let pidversion: UInt32

    /// An `audit_token_t` is eight opaque words; the ones this helper reads are 1, the
    /// effective uid, 5, the pid, and 7, the pid version.
    init(_ token: audit_token_t) {
        uid = token.val.1
        pid = Int32(bitPattern: token.val.5)
        pidversion = token.val.7
    }

    /// This process, from its own task port. A pid version of zero means the kernel refused the
    /// token and this process cannot name its own incarnation.
    static var current: ReconProcessIncarnation {
        ReconProcessIncarnation(pid: getpid(), uid: geteuid(), pidversion: Incarnation.pidversion)
    }

    /// The peer connected to this socket, as the kernel recorded it when it connected.
    static func peer(of descriptor: Int32) -> ReconProcessIncarnation? {
        var token = audit_token_t()
        var length = socklen_t(MemoryLayout<audit_token_t>.size)
        guard getsockopt(descriptor, SOL_LOCAL, LOCAL_PEERTOKEN, &token, &length) == 0,
            length == socklen_t(MemoryLayout<audit_token_t>.size)
        else { return nil }
        return ReconProcessIncarnation(token)
    }

    private init(pid: Int32, uid: UInt32, pidversion: UInt32) {
        self.pid = pid
        self.uid = uid
        self.pidversion = pidversion
    }
}
