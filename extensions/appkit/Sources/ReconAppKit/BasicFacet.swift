import ReconExtension
import ReconNativeSupport

/// What every kind of object reports about itself: its class, where it is, and what it is
/// attached to. The members a record carries are the ones its kind has.
@ReconModel
struct BasicFacet: Codable, Equatable {
    var status: StatusName = .value
    var reason: String? = nil
    var `class`: String? = nil
    var superclasses: [String]? = nil
    var title: TextValue? = nil
    var identifier: TextValue? = nil
    var frame: Rect? = nil
    var bounds: Rect? = nil
    var normalizedFrame: NormalizedFrame? = nil
    var accessibilityIdentifier: AccessibilityValue? = nil
    var accessibilityRole: AccessibilityValue? = nil
    var visible: Bool? = nil
    var key: Bool? = nil
    var main: Bool? = nil
    var windowNumber: IntegerValue? = nil
    var contentView: Reference? = nil
    var childWindows: [String]? = nil
    var sheet: Reference? = nil
    var topmostView: Reference? = nil
    var flipped: Bool? = nil
    var hidden: Bool? = nil
    var alpha: NumberValue? = nil
    var opacity: NumberValue? = nil
    var childCount: Int? = nil
    var superview: Reference? = nil
    var window: Reference? = nil
    var superlayer: Reference? = nil
    var owningView: Reference? = nil
    var layerState: String? = nil
    var hostingBoundary: HostingBoundary? = nil
    var enabled: Bool? = nil
    var editable: Bool? = nil
    var cellType: Int? = nil
    var state: Int? = nil

    /// A getter that raised fails the facet that asked for it, and nothing else.
    static let raisedGetter = BasicFacet(status: .failed, reason: "target-getter-exception")

    static func describing(class name: String, superclasses: [String]) -> BasicFacet {
        BasicFacet(class: name, superclasses: superclasses)
    }
}

extension Work {
    func basicFacet(of object: AnyObject) -> BasicFacet {
        do {
            return try basic(object)
        } catch {
            return .raisedGetter
        }
    }

    private func basic(_ object: AnyObject) throws -> BasicFacet {
        var facet = BasicFacet.describing(
            class: bounded(ReconNative.className(of: object)),
            superclasses: ReconNative.superclassNames(of: object, limit: 32).map(bounded))
        switch ReconNative.kind(of: object) {
        case .window: try describeWindow(object, into: &facet)
        case .view: try describeView(object, into: &facet)
        case .layer: try describeLayer(object, into: &facet)
        case .layoutGuide: try describeGuide(object, into: &facet)
        case .cell: try describeCell(object, into: &facet)
        default: break
        }
        return facet
    }

    private func describeWindow(_ window: AnyObject, into facet: inout BasicFacet) throws {
        facet.title = TextValue(try text(.title, of: window))
        facet.identifier = TextValue(try text(.identifier, of: window))
        let frame = try rect(.windowFrame, of: window) ?? .null
        facet.frame = Rect(frame, space: "global-screen", orientation: "bottom-left")
        let scale = try number(.windowBackingScaleFactor, of: window) ?? .nan
        let display = try screenNumber(of: try related(.windowScreen, of: window))
        let height = ReconNative.primaryScreenHeight
        facet.normalizedFrame = NormalizedFrame(
            screenTopLeft: .screenTopLeft(frame, primaryScreenHeight: height),
            backingScaleFactor: NumberValue(scale), displayId: DisplayIdentifier(display),
            transform: ScreenTransform(
                displayId: display, scale: scale, primaryScreenHeight: height))
        try describeAccessibility(window, into: &facet)
        facet.visible = try flag(.windowVisible, of: window)
        facet.key = try flag(.windowKey, of: window)
        facet.main = try flag(.windowMain, of: window)
        let number = try integer(.windowNumber, of: window) ?? 0
        facet.windowNumber = IntegerValue(number > 0 ? number : nil)
        let content = try related(.windowContentView, of: window)
        facet.contentView = Reference(reference(content))
        facet.childWindows = try childWindows(of: window)
        facet.sheet = try reference(.windowAttachedSheet, of: window)
        facet.topmostView = Reference(reference(try topmostView(of: window, from: content)))
    }

    private func childWindows(of window: AnyObject) throws -> [String] {
        var handles: [String] = []
        for child in try list(.windowChildWindows, of: window) {
            if handles.count >= limit {
                note(truncated: "child-windows:limit")
                break
            }
            if !consume() { break }
            if let handle = reference(child) { handles.append(handle) }
        }
        return handles
    }

    /// The outermost view the window still owns: a frame chain that leaves the window stops.
    private func topmostView(of window: AnyObject, from content: AnyObject?) throws -> AnyObject? {
        var top = content
        while let current = top, let parent = try related(.viewSuperview, of: current),
            try related(.viewWindow, of: parent) === window, consume()
        {
            top = parent
        }
        return top
    }

    private func describeView(_ view: AnyObject, into facet: inout BasicFacet) throws {
        let parent = try related(.viewSuperview, of: view)
        facet.identifier = TextValue(try text(.identifier, of: view))
        let parentFlipped = try parent.map { try flag(.viewFlipped, of: $0) ?? false }
        facet.frame = Rect(
            try rect(.viewFrame, of: view) ?? .null, space: "superview",
            orientation: parentFlipped.map { $0 ? "top-left" : "bottom-left" } ?? "unattached")
        let flipped = try flag(.viewFlipped, of: view) ?? false
        facet.bounds = Rect(
            try rect(.viewBounds, of: view) ?? .null, space: "self",
            orientation: flipped ? "top-left" : "bottom-left")
        facet.flipped = flipped
        facet.hidden = try flag(.viewHidden, of: view)
        facet.alpha = NumberValue(try number(.viewAlpha, of: view) ?? .nan)
        facet.childCount = try list(.viewSubviews, of: view).count
        facet.superview = Reference(reference(parent))
        let window = try related(.viewWindow, of: view)
        facet.window = Reference(reference(window))
        facet.normalizedFrame = try viewNormalizedFrame(of: view, in: window)
        try describeAccessibility(view, into: &facet)
        let name = ReconNative.className(of: view)
        if name.contains("HostingView") || name.contains("SwiftUI") {
            facet.hostingBoundary = HostingBoundary(
                evidence: "runtime-class-name-heuristic", swiftUIContent: "not-inspected")
        }
    }

    private func viewNormalizedFrame(of view: AnyObject, in window: AnyObject?) throws
        -> NormalizedFrame
    {
        guard let window else { return .absent }
        let inWindow = try value(ReconNative.rectInWindow(of: view)) ?? .null
        let onScreen = try value(ReconNative.screenRect(of: window, inWindow)) ?? .null
        let height = ReconNative.primaryScreenHeight
        let scale = try number(.windowBackingScaleFactor, of: window) ?? .nan
        let display = try screenNumber(of: try related(.windowScreen, of: window))
        return NormalizedFrame(
            window: Rect(inWindow, space: "window", orientation: "bottom-left"),
            screen: Rect(onScreen, space: "global-screen", orientation: "bottom-left"),
            screenTopLeft: .screenTopLeft(onScreen, primaryScreenHeight: height),
            backingScaleFactor: NumberValue(scale), displayId: DisplayIdentifier(display),
            transform: ScreenTransform(displayId: display, scale: scale, primaryScreenHeight: height))
    }

    private func describeLayer(_ layer: AnyObject, into facet: inout BasicFacet) throws {
        facet.frame = Rect(
            try rect(.layerFrame, of: layer) ?? .null, space: "parent-model-layer",
            orientation: "layer-geometry")
        facet.bounds = Rect(
            try rect(.layerBounds, of: layer) ?? .null, space: "self-model-layer",
            orientation: "layer-geometry")
        facet.hidden = try flag(.layerHidden, of: layer)
        facet.opacity = NumberValue(try number(.layerOpacity, of: layer) ?? .nan)
        facet.childCount = try list(.layerSublayers, of: layer).count
        facet.superlayer = try reference(.layerSuperlayer, of: layer)
        facet.layerState = "model"
    }

    private func describeGuide(_ guide: AnyObject, into facet: inout BasicFacet) throws {
        facet.identifier = TextValue(try text(.identifier, of: guide))
        let owner = try related(.guideOwningView, of: guide)
        let flipped = try owner.map { try flag(.viewFlipped, of: $0) ?? false } ?? false
        facet.frame = Rect(
            try rect(.guideFrame, of: guide) ?? .null, space: "owning-view",
            orientation: flipped ? "top-left" : "bottom-left")
        facet.owningView = Reference(reference(owner))
    }

    private func describeCell(_ cell: AnyObject, into facet: inout BasicFacet) throws {
        facet.enabled = try flag(.cellEnabled, of: cell)
        facet.editable = try flag(.cellEditable, of: cell)
        facet.cellType = try integer(.cellType, of: cell)
        facet.state = try integer(.cellState, of: cell)
    }

    private func screenNumber(of screen: AnyObject?) throws -> Int? {
        guard let screen else { return nil }
        return try integer(.screenNumber, of: screen)
    }

    private func describeAccessibility(_ object: AnyObject, into facet: inout BasicFacet) throws {
        facet.accessibilityIdentifier = try accessibility(.accessibilityIdentifier, of: object)
        facet.accessibilityRole = try accessibility(.accessibilityRole, of: object)
    }
}
