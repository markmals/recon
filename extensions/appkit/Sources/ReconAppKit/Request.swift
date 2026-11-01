import ReconExtension
import ReconNativeSupport

/// Which groups of facts a record carries.
enum FieldGroup: String, CaseIterable {
    case basic, appearance, font, layout, layer, cell
}

/// The reads this extension exports, which are also the request's published operations.
enum Operation: String {
    case windows, tree, node, find, fonts
}

/// What a client asked one read to do, after the checks proposal.0005 publishes.
///
/// Every member is accepted by every read, as the helper's request validation always has; a
/// read ignores what it has no use for.
struct InspectionRequest {
    enum Traversal: String {
        case view, layer
    }

    let operation: Operation
    let root: String?
    let windowId: Int?
    let depth: Int
    let limit: Int
    let fields: [FieldGroup]
    let kind: Traversal
    let className: String?
    let identifier: String?

    init(
        _ operation: Operation, root: String?, windowId: Int?, depth: Int, limit: Int,
        fields: [String]?, kind: String?, className: String?, identifier: String?
    ) throws(ReconActionError) {
        guard (0...64).contains(depth), (1...Work.visitLimit).contains(limit),
            let groups = Self.groups(fields)
        else { throw .invalidArgument("Invalid depth, limit, or fields", stage: "bounds") }
        let traversal = try Self.traversal(kind)
        if let root, !ReconHandleRegistry.isWellFormed(root) {
            throw .invalidArgument("Malformed native handle", stage: "root")
        }
        if operation == .node, root == nil {
            throw .invalidArgument("Native handle is required", stage: "root")
        }
        if let windowId, !(1...Int(UInt32.max)).contains(windowId) {
            throw .invalidArgument(
                "WindowServer ID must be a positive 32-bit integer", stage: "window")
        }
        if operation == .find, className == nil, identifier == nil {
            throw .invalidArgument("Find requires an exact class or identifier", stage: "filters")
        }
        if traversal == .layer, root == nil, windowId == nil {
            throw .invalidArgument(
                "Layer traversal requires a native root or WindowServer window", stage: "scope")
        }
        self.operation = operation
        self.root = root
        self.windowId = windowId
        self.depth = depth
        self.limit = limit
        self.fields = groups
        self.kind = traversal
        self.className = className
        self.identifier = identifier
    }

    var traversesLayers: Bool { kind == .layer }

    /// A node matches a find when every filter it names matches exactly.
    func matches(_ object: AnyObject) throws -> Bool {
        if let className, ReconNative.className(of: object) != className { return false }
        guard let identifier else { return true }
        let field: ReconStringField
        switch ReconNative.kind(of: object) {
        case .layer: field = .layerName
        case .view: field = .identifier
        default: return false
        }
        switch ReconNative.string(field, of: object) {
        case .value(let actual): return actual == identifier
        case .unavailable: return false
        case .failed: throw TargetGetterRaised()
        }
    }

    private static func traversal(_ kind: String?) throws(ReconActionError) -> Traversal {
        guard let kind else { return .view }
        guard let traversal = Traversal(rawValue: kind) else {
            throw .invalidArgument("Traversal kind must be view or layer", stage: "kind")
        }
        return traversal
    }

    private static func groups(_ fields: [String]?) -> [FieldGroup]? {
        guard let fields else { return [.basic] }
        guard !fields.isEmpty else { return nil }
        var groups: [FieldGroup] = []
        for name in fields {
            guard let group = FieldGroup(rawValue: name), !groups.contains(group) else {
                return nil
            }
            groups.append(group)
        }
        return groups
    }
}
