import Foundation
import ReconExtensionRuntime
import ReconNativeSupport

/// The helper's assembled, closed action set. Assembly failure is a startup failure, not a
/// crash: the server refuses to start rather than serving an extension set it could not build.
private let runtime = try? ReconRuntime(assembling: assembledExtensions)

@_cdecl("recon_runtime_ready")
public func reconRuntimeReady() -> Bool { runtime != nil }

/// Opens the native session an inspecting action's handles belong to. The server calls this
/// under the same lock it opens access with.
@_cdecl("recon_runtime_session_open")
public func reconRuntimeSessionOpen(
    _ sessionId: UnsafePointer<CChar>, _ helperBuild: UnsafePointer<CChar>,
    _ protocolVersion: UInt64
) {
    ReconNativeSession.open(
        identifier: String(cString: sessionId), helperBuild: String(cString: helperBuild),
        protocolVersion: Int(protocolVersion))
}

/// Revokes that session's handles. Objects an in-flight read still holds stay alive; only the
/// naming of them ends.
@_cdecl("recon_runtime_session_close")
public func reconRuntimeSessionClose(_ sessionId: UnsafePointer<CChar>) {
    ReconNativeSession.close(identifier: String(cString: sessionId))
}

/// Invokes one action for the native server and answers the complete response object.
///
/// The returned pointer is a +1 retained `NSData`; the Objective-C caller takes ownership with
/// `__bridge_transfer`. The blocks are retained by the scheduled work for as long as it needs
/// them, which can outlast this call after a timed-out wait.
@_cdecl("recon_runtime_invoke")
public func reconRuntimeInvoke(
    _ operation: UnsafePointer<CChar>, _ payload: UnsafeRawPointer, _ length: Int,
    _ remainingNanoseconds: UInt64,
    _ begin: @escaping @convention(block) () -> Bool,
    _ isRevoked: @escaping @convention(block) () -> Bool,
    _ settled: @escaping @convention(block) () -> Void
) -> UnsafeMutableRawPointer {
    guard let runtime else {
        settled()
        return response(
            .failure(
                ReconFailure(
                    code: "INSPECTION_UNAVAILABLE", stage: "assembly",
                    message: "Extension assembly failed")))
    }
    let access = ServerAccess(begin: begin, isRevoked: isRevoked, settled: settled).access
    let outcome = runtime.invoke(
        "appkit", String(cString: operation), payload: Data(bytes: payload, count: length),
        access: access, deadline: .now + .nanoseconds(Int64(clamping: remainingNanoseconds)))
    return response(outcome)
}

/// The server's blocks are called from the main thread and from the waiting server thread; each
/// one takes the server's own lock, so sharing them is safe even though blocks are not Sendable.
private struct ServerAccess: @unchecked Sendable {
    let begin: @convention(block) () -> Bool
    let isRevoked: @convention(block) () -> Bool
    let settled: @convention(block) () -> Void

    var access: ReconAccess {
        ReconAccess(begin: { begin() }, isRevoked: { isRevoked() }, settled: { settled() })
    }
}

/// The AppKit protocol answers one flat object: `ok` beside the result's own members.
private func response(_ outcome: ReconOutcome) -> UnsafeMutableRawPointer {
    Unmanaged.passRetained(encoded(outcome) as NSData).toOpaque()
}

private func encoded(_ outcome: ReconOutcome) -> Data {
    switch outcome {
    case .success(let result) where result.first == UInt8(ascii: "{"):
        var response = Data(#"{"ok":true"#.utf8)
        if result.count > 2 { response.append(UInt8(ascii: ",")) }
        response.append(result.dropFirst())
        return response
    case .success:
        return encoded(
            .failure(
                ReconFailure(
                    code: "INSPECTION_FAILED", stage: "encoding",
                    message: "Action result is not an object")))
    case .failure(let failure):
        var error: [String: Any] = [
            "code": failure.code, "stage": failure.stage, "message": failure.message,
        ]
        if let mayContinue = failure.targetExecutionMayContinue {
            error["targetExecutionMayContinue"] = mayContinue
        }
        return (try? JSONSerialization.data(withJSONObject: ["ok": false, "error": error]))
            ?? Data(#"{"ok":false}"#.utf8)
    }
}
