import ReconExtension
import ReconNativeSupport

/// Recon's AppKit inspector: the native reads `recon ui windows --session` and `recon ui view`
/// publish, run inside the inspected application on its own main thread.
///
/// Every read takes the same scope, bound, and filter members, as the helper's request contract
/// always has; a read ignores the ones it has no use for. Their observations are the ones
/// proposal.0005 specifies, unchanged.
@ReconExports("appkit")
@MainActor
public enum AppKitInspector {
    @ReconAction
    static func windows(
        root: String? = nil, windowId: Int? = nil, depth: Int = 3, limit: Int = 200,
        fields: [String]? = nil, kind: String? = nil, `class` className: String? = nil,
        identifier: String? = nil, context: ReconContext
    ) throws -> WindowsResponse {
        let work = try work(
            .windows, root, windowId, depth, limit, fields, kind, className, identifier, context)
        return try work.windowsObservation()
    }

    @ReconAction
    static func tree(
        root: String? = nil, windowId: Int? = nil, depth: Int = 3, limit: Int = 200,
        fields: [String]? = nil, kind: String? = nil, `class` className: String? = nil,
        identifier: String? = nil, context: ReconContext
    ) throws -> NodesResponse {
        let work = try work(
            .tree, root, windowId, depth, limit, fields, kind, className, identifier, context)
        return try work.nodesObservation()
    }

    @ReconAction
    static func node(
        root: String? = nil, windowId: Int? = nil, depth: Int = 3, limit: Int = 200,
        fields: [String]? = nil, kind: String? = nil, `class` className: String? = nil,
        identifier: String? = nil, context: ReconContext
    ) throws -> NodeResponse {
        let work = try work(
            .node, root, windowId, depth, limit, fields, kind, className, identifier, context)
        return try work.nodeObservation()
    }

    @ReconAction
    static func find(
        root: String? = nil, windowId: Int? = nil, depth: Int = 3, limit: Int = 200,
        fields: [String]? = nil, kind: String? = nil, `class` className: String? = nil,
        identifier: String? = nil, context: ReconContext
    ) throws -> NodesResponse {
        let work = try work(
            .find, root, windowId, depth, limit, fields, kind, className, identifier, context)
        return try work.nodesObservation()
    }

    @ReconAction
    static func fonts(
        root: String? = nil, windowId: Int? = nil, depth: Int = 3, limit: Int = 200,
        fields: [String]? = nil, kind: String? = nil, `class` className: String? = nil,
        identifier: String? = nil, context: ReconContext
    ) throws -> FontsResponse {
        let work = try work(
            .fonts, root, windowId, depth, limit, fields, kind, className, identifier, context)
        return try work.fontsObservation()
    }

    private static func work(
        _ operation: Operation, _ root: String?, _ windowId: Int?, _ depth: Int, _ limit: Int,
        _ fields: [String]?, _ kind: String?, _ className: String?, _ identifier: String?,
        _ context: ReconContext
    ) throws -> Work {
        let request = try InspectionRequest(
            operation, root: root, windowId: windowId, depth: depth, limit: limit, fields: fields,
            kind: kind, className: className, identifier: identifier)
        guard let session = ReconNativeSession.current else {
            throw ReconActionError.unavailable("No native session is open", stage: "read")
        }
        return Work(request: request, session: session, deadline: context.deadline)
    }
}

extension Work {
    /// Every window of the application, each matched to the WindowServer window it is on screen
    /// as, where exactly one matches.
    func windowsObservation() throws -> WindowsResponse {
        let scope = try scope()
        var records: [NodeRecord] = []
        let mapped = try mappedIdentifiers(of: scope.windows)
        for window in scope.windows {
            if !consume() { break }
            var record = try record(window)
            let identifiers = mapped.filter { $0.window === window }.map(\.identifier)
            record.windowId = WindowIdentifier(identifiers.count == 1 ? identifiers[0] : nil)
            if identifiers.count == 1 {
                record.mapping = .windowNumber
            } else if identifiers.count > 1 {
                record.mapping = .ambiguous
            }
            records.append(record)
            emitted += 1
            if records.count >= limit {
                note(truncated: "windows:limit")
                break
            }
        }
        return WindowsResponse(
            windows: records, observation: observation(fields: fieldNames, kind: "window"),
            coverage: try coverage())
    }

    /// The WindowServer windows this process owns on screen, paired with the native window each
    /// one resolves to.
    private func mappedIdentifiers(of windows: [AnyObject]) throws
        -> [(window: AnyObject, identifier: Int)]
    {
        let pid = ReconProcessIdentity.current.pid
        var mapped: [(AnyObject, Int)] = []
        for entry in ReconNative.windowServerInventory() {
            if !consume() { break }
            guard Int(entry.ownerPid) == pid, entry.onscreen else { continue }
            let identifier = Int(entry.number)
            guard let resolved = ReconNative.applicationWindow(number: identifier),
                let member = windows.first(where: { $0 === resolved })
            else { continue }
            mapped.append((member, identifier))
        }
        return mapped
    }

    func nodeObservation() throws -> NodeResponse {
        let scope = try scope()
        guard let root = scope.root else {
            throw ReconActionError.invalidArgument("A native handle is required", stage: "node")
        }
        let record = try record(root)
        emitted = 1
        return NodeResponse(
            node: record,
            observation: observation(
                fields: fieldNames,
                kind: ReconNative.kind(of: root).publishedName ?? ""),
            coverage: try coverage())
    }

    func nodesObservation() throws -> NodesResponse {
        let traversal = try traverse(try scope())
        return NodesResponse(
            nodes: traversal.records, windowRoots: traversal.windowRoots,
            observation: observation(fields: fieldNames, kind: request.kind.rawValue),
            coverage: try coverage())
    }

    func fontsObservation() throws -> FontsResponse {
        let traversal = try traverse(try scope())
        return FontsResponse(
            fonts: traversal.fonts.all, windowRoots: traversal.windowRoots,
            observation: observation(fields: ["font"], kind: request.kind.rawValue),
            coverage: try coverage())
    }

    private var fieldNames: [String] { request.fields.map(\.rawValue) }
}
