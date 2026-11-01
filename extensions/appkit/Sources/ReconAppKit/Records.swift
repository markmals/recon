import ReconExtension
import ReconNativeSupport

/// One object's handle, kind, relationships, and the facets the read asked for.
///
/// A record carries only what its kind has: a window has no bounds, a layer has no cell. The
/// members a traversal adds — depth, and whether this row is a repeat — appear only there.
@ReconModel
struct NodeRecord: Codable, Equatable {
    let handle: String
    let kind: String
    let depth: Int?
    let reference: Bool?
    let relationships: Relationships?
    var windowId: WindowIdentifier?
    var mapping: WindowMappingEvidence?
    var basic: BasicFacet?
    var appearance: AppearanceFacet?
    var font: FontFacet?
    var layout: LayoutFacet?
    var layer: LayerFacet?
    var cell: CellFacet?
}

/// The objects this one is attached to, by handle.
@ReconModel
struct Relationships: Codable, Equatable {
    let status: StatusName?
    let superview: Reference?
    let window: Reference?
    let superlayer: Reference?
    let parentWindow: Reference?
    let contentView: Reference?
    let owningView: Reference?

    static let failed = Relationships(
        status: .failed, superview: nil, window: nil, superlayer: nil, parentWindow: nil,
        contentView: nil, owningView: nil)
    static let none = Relationships(
        status: nil, superview: nil, window: nil, superlayer: nil, parentWindow: nil,
        contentView: nil, owningView: nil)
}

/// An accessibility property as the object itself reports it. An empty identifier or role is
/// AppKit's way of exposing none, so it is reported as absent rather than as a value that would
/// match every other object exposing none.
@ReconModel
struct AccessibilityValue: Codable, Equatable {
    let status: StatusName
    let value: String?
    let read: String?

    static let unsupported = AccessibilityValue(status: .unsupported, value: nil, read: nil)

    init(status: StatusName, value: String?, read: String?) {
        self.status = status
        self.value = value
        self.read = read
    }

    init(_ text: String?) {
        let present = !(text ?? "").isEmpty
        status = present ? .value : .absent
        value = present ? text : nil
        read = "target-executed"
    }
}

/// SwiftUI content hosted in an AppKit view is a boundary Recon names but does not cross.
@ReconModel
struct HostingBoundary: Codable, Equatable {
    let evidence: String
    let swiftUIContent: String
}

extension Work {
    /// One object's record. An unsupported kind and an exhausted registry are both the end of
    /// the read: a record that cannot name its object is not an observation.
    func record(_ object: AnyObject, depth: Int? = nil) throws -> NodeRecord {
        guard let name = ReconNative.kind(of: object).publishedName else {
            throw Self.registryExhausted
        }
        var record = NodeRecord(
            handle: try requiredReference(object), kind: name, depth: depth, reference: nil,
            relationships: relationships(of: object), windowId: nil, mapping: nil, basic: nil,
            appearance: nil, font: nil, layout: nil, layer: nil, cell: nil)
        for group in request.fields {
            // The walker this replaces abandons a record whose read ran out of time, and its
            // caller reports that as an exhausted registry.
            if expired { throw Self.registryExhausted }
            switch group {
            case .basic: record.basic = basicFacet(of: object)
            case .appearance: record.appearance = appearanceFacet(of: object)
            case .font: record.font = fontFacet(of: object)
            case .layout: record.layout = layoutFacet(of: object)
            case .layer: record.layer = layerFacet(of: object)
            case .cell: record.cell = cellFacet(of: object)
            }
        }
        return record
    }

    /// A repeated object is reported as a reference to the row that described it.
    func repeatedRecord(_ object: AnyObject, depth: Int) throws -> NodeRecord {
        NodeRecord(
            handle: try requiredReference(object),
            kind: ReconNative.kind(of: object).publishedName ?? "", depth: depth, reference: true,
            relationships: nil, windowId: nil, mapping: nil, basic: nil, appearance: nil,
            font: nil, layout: nil, layer: nil, cell: nil)
    }

    private func relationships(of object: AnyObject) -> Relationships {
        do {
            switch ReconNative.kind(of: object) {
            case .view:
                return Relationships(
                    status: nil, superview: try reference(.viewSuperview, of: object),
                    window: try reference(.viewWindow, of: object), superlayer: nil,
                    parentWindow: nil, contentView: nil, owningView: nil)
            case .layer:
                return Relationships(
                    status: nil, superview: nil, window: nil,
                    superlayer: try reference(.layerSuperlayer, of: object), parentWindow: nil,
                    contentView: nil, owningView: nil)
            case .window:
                return Relationships(
                    status: nil, superview: nil, window: nil, superlayer: nil,
                    parentWindow: try reference(.windowParentWindow, of: object),
                    contentView: try reference(.windowContentView, of: object), owningView: nil)
            case .layoutGuide:
                return Relationships(
                    status: nil, superview: nil, window: nil, superlayer: nil, parentWindow: nil,
                    contentView: nil, owningView: try reference(.guideOwningView, of: object))
            default:
                return .none
            }
        } catch {
            return .failed
        }
    }
}

extension ReconObjectKind {
    var publishedName: String? {
        switch self {
        case .window: "window"
        case .view: "view"
        case .layer: "layer"
        case .cell: "cell"
        case .layoutGuide: "layoutGuide"
        default: nil
        }
    }
}
