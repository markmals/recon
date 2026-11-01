import ReconExtension
import ReconNativeSupport

/// A WindowServer window identifier, or the reason this window has none.
struct WindowIdentifier: Codable, Equatable, ReconSchema {
    let number: Int?

    init(_ number: Int?) { self.number = number }

    init(from decoder: any Decoder) throws {
        let container = try decoder.singleValueContainer()
        number = try? container.decode(Int.self)
    }

    func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        if let number {
            try container.encode(number)
        } else {
            try container.encode(Status(.unavailable))
        }
    }

    static var reconSchema: ReconSchemaNode { .oneOf([.of(Int.self), .status(.unavailable)]) }
}

/// How a native window was matched to a WindowServer window, or that it could not be matched
/// to exactly one.
@ReconModel
struct WindowMappingEvidence: Codable, Equatable {
    let method: String?
    let evidence: String?
    let status: StatusName?

    static let ambiguous = WindowMappingEvidence(
        method: nil, evidence: nil, status: .ambiguous)
    static let windowNumber = WindowMappingEvidence(
        method: "NSApplication.windowWithWindowNumber",
        evidence: "WindowServer-owner-pid-and-NSApp-windows-membership", status: nil)
}

/// What one read is scoped to: the application's windows, the selected window, and the root the
/// client named.
struct Scope {
    let windows: [AnyObject]
    let window: AnyObject?
    let root: AnyObject?
}

extension Work {
    /// Resolves the request's scope against live objects, before any traversal starts.
    func scope() throws -> Scope {
        let needsWindows =
            request.windowId != nil || request.root == nil || request.operation == .windows
        let windows = needsWindows ? try applicationWindows() : []
        let window = try request.windowId.map { try mappedWindow($0, in: windows) }
        let root = try resolvedRoot()
        if let root, let window, try windowOwning(root) !== window {
            throw ReconActionError.staleReference(
                "Native object is no longer in the selected window", stage: "scope")
        }
        return Scope(windows: windows, window: window, root: root)
    }

    func applicationWindows() throws -> [AnyObject] {
        try value(ReconNative.applicationWindows()) ?? []
    }

    private func resolvedRoot() throws -> AnyObject? {
        guard let handle = request.root else { return nil }
        guard let object = session.handles.object(for: handle) else {
            throw ReconActionError.staleReference(
                "Native handle is no longer live", stage: "root")
        }
        return object
    }

    /// The one live, owned, on-screen native window a WindowServer identifier names. Every way
    /// that can fail names which step failed, because they mean different things.
    private func mappedWindow(_ identifier: Int, in windows: [AnyObject]) throws -> AnyObject {
        let owned = ReconNative.windowServerInventory().contains {
            Int($0.number) == identifier && Int($0.ownerPid) == ReconProcessIdentity.current.pid
                && $0.onscreen
        }
        guard owned else { throw Self.unmapped("window-server-owner") }
        guard let resolved = ReconNative.applicationWindow(number: identifier) else {
            throw Self.unmapped("appkit-window-number")
        }
        guard windows.contains(where: { $0 === resolved }) else {
            throw Self.unmapped("app-window-membership")
        }
        return resolved
    }

    private static func unmapped(_ stage: String) -> ReconActionError {
        .unavailable(
            "No live owned native window matches the WindowServer ID (\(stage))", stage: stage)
    }

    /// The window an object belongs to, which is what keeps a selected window's scope honest.
    func windowOwning(_ object: AnyObject) throws -> AnyObject? {
        switch ReconNative.kind(of: object) {
        case .window: return object
        case .view: return try related(.viewWindow, of: object)
        case .layoutGuide:
            return try related(.guideOwningView, of: object)
                .flatMap { try related(.viewWindow, of: $0) }
        case .layer: return try hostingWindow(ofLayer: object)
        default: return nil
        }
    }

    /// A layer belongs to the window of the nearest view that backs it.
    private func hostingWindow(ofLayer layer: AnyObject) throws -> AnyObject? {
        var current: AnyObject? = layer
        while let parent = current {
            if let view = try related(.layerDelegate, of: parent),
                ReconNative.kind(of: view) == .view,
                try related(.viewLayer, of: view) === parent
            {
                return try related(.viewWindow, of: view)
            }
            current = try related(.layerSuperlayer, of: parent)
        }
        return nil
    }

    /// Where a traversal starts: a window traverses its content view, and a layer traversal
    /// starts at that view's layer.
    func traversalRoot(_ object: AnyObject) throws -> AnyObject? {
        var root: AnyObject? = object
        if ReconNative.kind(of: object) == .window {
            root = try related(.windowContentView, of: object)
        }
        guard request.traversesLayers, let view = root, ReconNative.kind(of: view) == .view else {
            return root
        }
        return try related(.viewLayer, of: view)
    }

    func children(of object: AnyObject) throws -> [AnyObject] {
        if request.traversesLayers {
            return ReconNative.kind(of: object) == .layer
                ? try list(.layerSublayers, of: object) : []
        }
        return ReconNative.kind(of: object) == .view ? try list(.viewSubviews, of: object) : []
    }
}
