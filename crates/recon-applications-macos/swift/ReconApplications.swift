// The Objective-C frameworks this crate needs — NSWorkspace, NSRunningApplication and
// NSFileManager — reached from Swift and exported to Rust over a C ABI.
//
// Everything here is a thin, explicit boundary: one framework call per export, no policy, and no
// state kept between calls. The decisions — which running applications count, what a refusal
// means, how long an open may take — stay in Rust, so that moving a framework call across the
// language boundary does not quietly move the behaviour with it.
//
// Ownership is explicit in both directions. A `const char *` argument belongs to the caller for
// the duration of the call. Every returned string is allocated here and belongs to the caller,
// which frees it with `recon_applications_string_free` and nothing else.

import AppKit
import Foundation

/// A result the caller owns: a NUL-terminated UTF-8 copy of `text`.
private func exported(_ text: String) -> UnsafeMutablePointer<CChar>? {
    var utf8 = Array(text.utf8CString)
    let buffer = UnsafeMutablePointer<CChar>.allocate(capacity: utf8.count)
    utf8.withUnsafeBufferPointer { source in
        buffer.update(from: source.baseAddress!, count: source.count)
    }
    utf8.removeAll()
    return buffer
}

/// A JSON object or array the caller owns, or null when it could not be encoded.
private func exportedJSON(_ value: Any) -> UnsafeMutablePointer<CChar>? {
    guard let encoded = try? JSONSerialization.data(withJSONObject: value),
        let text = String(data: encoded, encoding: .utf8)
    else {
        return nil
    }
    return exported(text)
}

/// Releases a string this library returned. Passing anything else, or the same pointer twice, is
/// undefined.
@_cdecl("recon_applications_string_free")
public func reconApplicationsStringFree(_ text: UnsafeMutablePointer<CChar>?) {
    text?.deallocate()
}

/// Every running application that has a bundle, as `[{"path": "…", "pid": N}]`.
///
/// An application without a bundle URL or without a usable process ID is left out rather than
/// reported with a missing field: the caller matches these against bundles on disk.
@_cdecl("recon_applications_running")
public func reconApplicationsRunning() -> UnsafeMutablePointer<CChar>? {
    let running = NSWorkspace.shared.runningApplications.compactMap { application -> [String: Any]? in
        guard let path = application.bundleURL?.path, application.processIdentifier >= 0 else {
            return nil
        }
        return ["path": path, "pid": Int(application.processIdentifier)]
    }
    return exportedJSON(running)
}

/// The process ID of the frontmost application, or -1 when macOS reports none.
///
/// `NSWorkspace` learns of activation changes through run-loop notifications, so a caller that
/// never runs its run loop keeps seeing whichever application was frontmost when it started.
@_cdecl("recon_applications_frontmost_pid")
public func reconApplicationsFrontmostPid() -> Int32 {
    NSWorkspace.shared.frontmostApplication?.processIdentifier ?? -1
}

/// The display name macOS shows for `path`, which may be empty when it has none.
@_cdecl("recon_applications_display_name")
public func reconApplicationsDisplayName(_ path: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>? {
    exported(FileManager.default.displayName(atPath: String(cString: path)))
}

/// Opens exactly `path`, and reports what happened as one JSON object.
///
/// `{"pid": N}` when macOS opened it; otherwise `{"reason": "…", "detail": "…"}` naming the step
/// that did not happen. A negative `timeoutSeconds` waits for the callback indefinitely.
///
/// Never creates a new instance and never substitutes a running copy from another location, so
/// the process this answers with belongs to the bundle the caller named.
@_cdecl("recon_applications_open")
public func reconApplicationsOpen(
    _ path: UnsafePointer<CChar>,
    _ activates: Bool,
    _ timeoutSeconds: Double
) -> UnsafeMutablePointer<CChar>? {
    let url = URL(fileURLWithPath: String(cString: path))
    let configuration = NSWorkspace.OpenConfiguration()
    configuration.activates = activates
    configuration.createsNewApplicationInstance = false
    configuration.allowsRunningApplicationSubstitution = false

    let outcome = Outcome()
    let finished = DispatchSemaphore(value: 0)
    NSWorkspace.shared.openApplication(at: url, configuration: configuration) { opened, error in
        outcome.record(opened: opened, error: error)
        finished.signal()
    }
    if timeoutSeconds < 0 {
        finished.wait()
    } else if finished.wait(timeout: .now() + timeoutSeconds) == .timedOut {
        return exportedJSON(["reason": "timeout"])
    }
    return exportedJSON(outcome.reported())
}

/// What the open callback saw, read back on the thread that waited for it.
private final class Outcome: @unchecked Sendable {
    private let lock = NSLock()
    private var pid: Int32?
    private var refusal: String?

    func record(opened: NSRunningApplication?, error: Error?) {
        lock.lock()
        defer { lock.unlock() }
        if let error {
            refusal = error.localizedDescription
        } else if let opened {
            pid = opened.processIdentifier
        }
    }

    func reported() -> [String: Any] {
        lock.lock()
        defer { lock.unlock() }
        if let refusal {
            return ["reason": "refused", "detail": refusal]
        }
        guard let pid else {
            return ["reason": "not-opened"]
        }
        return pid >= 0 ? ["pid": Int(pid)] : ["reason": "no-process-id"]
    }
}
