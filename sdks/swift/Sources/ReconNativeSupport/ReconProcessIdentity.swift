import Darwin
import Foundation
import ReconExtension

/// The target process incarnation an observation was read from.
///
/// An absent bundle identifier or path is published as `null` rather than omitted: the member
/// always exists, and its absence is a fact about the process.
@ReconModel
public struct ReconProcessIdentity: Codable, Sendable, Equatable {
    public let pid: Int
    public let pidversion: UInt32
    public let uid: UInt32
    public let bundleId: ReconNullable<String>
    public let bundlePath: ReconNullable<String>
    public let executablePath: ReconNullable<String>
    public let architecture: String
    public let osBuild: String
    public let osVersion: String

    /// This process, as the helper's lifecycle response reports it.
    public static var current: ReconProcessIdentity {
        let bundle = ProcessBundle.facts()
        return ReconProcessIdentity(
            pid: Int(getpid()), pidversion: Incarnation.pidversion, uid: geteuid(),
            bundleId: ReconNullable(bundle.identifier), bundlePath: ReconNullable(bundle.path),
            executablePath: ReconNullable(bundle.executablePath),
            architecture: Incarnation.architecture, osBuild: Incarnation.operatingSystemBuild,
            osVersion: ProcessInfo.processInfo.operatingSystemVersionString)
    }
}

/// What this process is, from the kernel rather than from anything the application says.
public enum Incarnation {
    /// The audit token's generation count, which separates this process from a later one that
    /// reuses its pid.
    public static var pidversion: UInt32 {
        var token = audit_token_t()
        var count = mach_msg_type_number_t(
            MemoryLayout<audit_token_t>.size / MemoryLayout<natural_t>.size)
        let read = withUnsafeMutablePointer(to: &token) {
            $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
                task_info(mach_task_self_, task_flavor_t(TASK_AUDIT_TOKEN), $0, &count)
            }
        }
        return read == KERN_SUCCESS ? token.val.7 : 0
    }

    static var architecture: String {
        var info = proc_archinfo()
        let size = Int32(MemoryLayout<proc_archinfo>.size)
        let read = proc_pidinfo(getpid(), PROC_PIDARCHINFO, 0, &info, size)
        guard read == size, info.p_cputype == CPU_TYPE_ARM64 else { return "unknown" }
        let subtype = info.p_cpusubtype & ~Int32(bitPattern: CPU_SUBTYPE_MASK)
        return subtype == CPU_SUBTYPE_ARM64E || subtype == 12 ? "arm64e" : "arm64"
    }

    static var operatingSystemBuild: String {
        var size = 0
        guard sysctlbyname("kern.osversion", nil, &size, nil, 0) == 0, size > 0 else { return "" }
        var value = [UInt8](repeating: 0, count: size)
        guard sysctlbyname("kern.osversion", &value, &size, nil, 0) == 0 else { return "" }
        return String(decoding: value.prefix(while: { $0 != 0 }), as: UTF8.self)
    }
}

/// The bundle belongs to the inspected application, which may answer for it however it likes,
/// so every one of these reads goes through the boundary and one raise leaves them all absent.
private enum ProcessBundle {
    static func facts() -> (path: String?, identifier: String?, executablePath: String?) {
        guard case .value(let bundle) = Message.object(Fields.mainBundle, to: Bundle.self) else {
            return (nil, nil, nil)
        }
        var reads = ReadSequence()
        let path = reads.optional(Message.text(Fields.bundlePath, to: bundle))
        let identifier = reads.optional(Message.text(Fields.bundleIdentifier, to: bundle))
        let executablePath = reads.optional(Message.text(Fields.executablePath, to: bundle))
        guard !reads.failed else { return (nil, nil, nil) }
        return (path.flatMap { $0.hasSuffix(".app") ? $0 : nil }, identifier, executablePath)
    }
}
