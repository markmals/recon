import Foundation
import ReconExtension
import ReconNativeSupport

@ReconModel
enum ReadStatus: String, Codable {
    case value, unavailable, failed
}

@ReconModel
struct StringRead: Codable {
    let status: ReadStatus
    let value: String?
}

@ReconModel
struct ViewRecord: Codable {
    let className: String
    let identifier: StringRead
    let toolTip: StringRead
}

@ReconModel
struct WindowRecord: Codable {
    let windowNumber: Int?
    let className: String
    let title: StringRead
    let views: [ViewRecord]
}

@ReconModel
public struct WindowsObservation: Codable {
    let windows: [WindowRecord]
    let truncated: Bool
    let invocations: Int
    let mainThread: Bool
}

/// Every target-object read goes through an allowlisted native-support primitive, which catches
/// Objective-C exceptions before returning here.
private func read(_ object: AnyObject, _ field: ReconStringField) -> StringRead {
    var value: NSString?
    switch ReconReadString(object, field, &value) {
    case .value: return StringRead(status: .value, value: value as String?)
    case .unavailable: return StringRead(status: .unavailable, value: nil)
    default: return StringRead(status: .failed, value: nil)
    }
}

@ReconExports("appkit")
@MainActor
public enum AppKitInspector {
    static var invocations = 0

    @ReconAction
    static func windows(limit: Int = 50, context: ReconContext) throws -> WindowsObservation {
        invocations += 1
        guard (1...25_000).contains(limit) else {
            throw ReconActionError.invalidArgument("limit must be in 1...25000")
        }
        var list: NSArray?
        guard ReconReadApplicationWindows(&list) == .value, let windows = list as? [AnyObject] else {
            throw ReconActionError.unavailable("NSApp.windows could not be read")
        }
        var records: [WindowRecord] = []
        var emitted = 0
        for window in windows {
            try context.checkCancellation()
            guard emitted < limit else { break }
            emitted += 1
            var views: [ViewRecord] = []
            for child in subviews(ofContentViewOf: window) where emitted < limit {
                emitted += 1
                views.append(
                    ViewRecord(
                        className: ReconClassName(child), identifier: read(child, .identifier),
                        toolTip: read(child, .toolTip)))
            }
            var number = 0
            let numbered = ReconReadWindowNumber(window, &number) == .value
            records.append(
                WindowRecord(
                    windowNumber: numbered ? number : nil, className: ReconClassName(window),
                    title: read(window, .title), views: views))
        }
        return WindowsObservation(
            windows: records, truncated: emitted >= limit && records.count < windows.count,
            invocations: invocations, mainThread: Thread.isMainThread)
    }

    private static func subviews(ofContentViewOf window: AnyObject) -> [AnyObject] {
        var content: AnyObject?
        guard ReconReadContentView(window, &content) == .value, let content else { return [] }
        var children: NSArray?
        guard ReconReadSubviews(content, &children) == .value else { return [] }
        return children as? [AnyObject] ?? []
    }
}
