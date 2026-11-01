import Foundation
import ReconExtensionRuntime

/// The shapes a response takes on the wire: a refusal, or one action's object behind `ok`.
enum ReconEnvelope {
    static func error(_ code: String, _ stage: String, _ message: String) -> [String: Any] {
        ["ok": false, "error": ["code": code, "stage": stage, "message": message]]
    }

    static func serialized(_ response: [String: Any]) -> Data? {
        try? JSONSerialization.data(withJSONObject: response)
    }

    /// The AppKit protocol answers one flat object: `ok` beside the result's own members.
    static func response(_ outcome: ReconOutcome) -> Data {
        switch outcome {
        case .success(let result) where result.first == UInt8(ascii: "{"):
            var response = Data(#"{"ok":true"#.utf8)
            if result.count > 2 { response.append(UInt8(ascii: ",")) }
            response.append(result.dropFirst())
            return response
        case .success:
            return response(
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
            return serialized(["ok": false, "error": error]) ?? Data(#"{"ok":false}"#.utf8)
        }
    }
}
