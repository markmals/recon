import Foundation
import ReconAppKit
import ReconExtensionRuntime

/// The helper's assembled, closed action set. Assembly failure is a startup failure, not a crash.
private let runtime = try? ReconRuntime(assembling: [AppKitInspector.reconExtension])

@_cdecl("recon_runtime_ready")
public func reconRuntimeReady() -> Bool { runtime != nil }

/// Invokes one AppKit action for the native server and returns the complete response object.
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
    let access = UnsafeBlocks(begin: begin, isRevoked: isRevoked, settled: settled).access
    let response: Data
    if let runtime {
        let outcome = runtime.invoke(
            "appkit", String(cString: operation), payload: Data(bytes: payload, count: length),
            access: access, deadline: .now + .nanoseconds(Int64(clamping: remainingNanoseconds)))
        response = wireResponse(outcome)
    } else {
        settled()
        response = wireResponse(
            .failure(ReconFailureWire.unavailable("Extension assembly failed").failure))
    }
    return Unmanaged.passRetained(response as NSData).toOpaque()
}

/// The server's blocks are called from the main thread and the waiting server thread; each one
/// takes the server's own lock, so sharing them is safe even though blocks are not `Sendable`.
private struct UnsafeBlocks: @unchecked Sendable {
    let begin: @convention(block) () -> Bool
    let isRevoked: @convention(block) () -> Bool
    let settled: @convention(block) () -> Void

    var access: ReconAccess {
        ReconAccess(begin: { begin() }, isRevoked: { isRevoked() }, settled: { settled() })
    }
}

/// The existing AppKit protocol answers one flat object: `ok` beside the result's own members.
private func wireResponse(_ outcome: ReconOutcome) -> Data {
    switch outcome {
    case .success(let result) where result.first == UInt8(ascii: "{"):
        var response = Data(#"{"ok":true"#.utf8)
        if result.count > 2 { response.append(UInt8(ascii: ",")) }
        response.append(result.dropFirst())
        return response
    case .success:
        return wireResponse(
            .failure(ReconFailureWire.failed("AppKit action result is not an object").failure))
    case .failure(let failure):
        // AppKit actions are main-actor work; the existing protocol names their queue explicitly.
        let stage = failure.stage == "queue" ? "main-thread-queue" : failure.stage
        var error: [String: Any] = ["code": failure.code, "stage": stage, "message": failure.message]
        if let mayContinue = failure.targetExecutionMayContinue {
            error["targetExecutionMayContinue"] = mayContinue
        }
        return (try? JSONSerialization.data(withJSONObject: ["ok": false, "error": error]))
            ?? Data(#"{"ok":false}"#.utf8)
    }
}

private enum ReconFailureWire {
    case unavailable(String), failed(String)

    var failure: ReconFailure {
        switch self {
        case .unavailable(let message): ReconFailure(code: "INSPECTION_UNAVAILABLE", stage: "assembly", message: message)
        case .failed(let message): ReconFailure(code: "INSPECTION_FAILED", stage: "encoding", message: message)
        }
    }
}
