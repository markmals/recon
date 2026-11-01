import ReconExtension
import ReconNativeSupport

/// A layer's model transform, which is sixteen numbers or none at all.
@ReconModel
struct TransformValue: Codable, Equatable {
    let status: StatusName
    let matrix: [Double]?

    init(_ matrix: [Double]) {
        let finite = matrix.allSatisfy(\.isFinite)
        status = finite ? .value : .nonFinite
        self.matrix = finite ? matrix : nil
    }
}

/// The model layer behind an object: what Core Animation would draw, not what is on screen.
@ReconModel
struct LayerFacet: Codable, Equatable {
    var status: StatusName = .value
    var reason: String? = nil
    var layerState: String? = nil
    var handle: Reference? = nil
    var frame: Rect? = nil
    var bounds: Rect? = nil
    var transform: TransformValue? = nil
    var contentsScale: Scalar? = nil
    var hidden: Bool? = nil
    var opacity: Scalar? = nil
    var cornerRadius: Scalar? = nil
    var maskedCorners: Int? = nil
    var borderWidth: Scalar? = nil
    var shadowOpacity: Scalar? = nil
    var shadowRadius: Scalar? = nil
    var masksToBounds: Bool? = nil
    var mask: Reference? = nil
    var backgroundColor: Color? = nil
    var borderColor: Color? = nil
    var shadowColor: Color? = nil

    static let unsupported = LayerFacet(status: .unsupported)
    static let absent = LayerFacet(status: .absent)
    static let raisedGetter = LayerFacet(status: .failed, reason: "target-getter-exception")
}

/// The cell a control draws with, or the cell itself.
@ReconModel
struct CellFacet: Codable, Equatable {
    var status: StatusName = .value
    var reason: String? = nil
    var handle: Reference? = nil
    var `class`: String? = nil
    var cellType: Int? = nil
    var state: Int? = nil
    var enabled: Bool? = nil
    var editable: Bool? = nil
    var font: FontDescriptor? = nil

    static let unsupported = CellFacet(status: .unsupported)
    static let absent = CellFacet(status: .absent)
    static let raisedGetter = CellFacet(status: .failed, reason: "target-getter-exception")
}

extension Work {
    func layerFacet(of object: AnyObject) -> LayerFacet {
        do {
            return try layer(object)
        } catch {
            return .raisedGetter
        }
    }

    private func layer(_ object: AnyObject) throws -> LayerFacet {
        let layer: AnyObject?
        switch ReconNative.kind(of: object) {
        case .view: layer = try related(.viewLayer, of: object)
        case .layer: layer = object
        case .window:
            layer = try related(.windowContentView, of: object)
                .flatMap { try related(.viewLayer, of: $0) }
        default: return .unsupported
        }
        guard let layer else { return .absent }
        return LayerFacet(
            layerState: "model", handle: Reference(reference(layer), absence: .unavailable),
            frame: Rect(
                try rect(.layerFrame, of: layer) ?? .null, space: "parent-model-layer",
                orientation: "layer-geometry"),
            bounds: Rect(
                try rect(.layerBounds, of: layer) ?? .null, space: "self-model-layer",
                orientation: "layer-geometry"),
            transform: TransformValue(try value(ReconNative.transform(ofLayer: layer)) ?? []),
            contentsScale: Scalar(try number(.layerContentsScale, of: layer) ?? .nan),
            hidden: try flag(.layerHidden, of: layer),
            opacity: Scalar(try number(.layerOpacity, of: layer) ?? .nan),
            cornerRadius: Scalar(try number(.layerCornerRadius, of: layer) ?? .nan),
            maskedCorners: try integer(.layerMaskedCorners, of: layer),
            borderWidth: Scalar(try number(.layerBorderWidth, of: layer) ?? .nan),
            shadowOpacity: Scalar(try number(.layerShadowOpacity, of: layer) ?? .nan),
            shadowRadius: Scalar(try number(.layerShadowRadius, of: layer) ?? .nan),
            masksToBounds: try flag(.layerMasksToBounds, of: layer),
            mask: try reference(.layerMask, of: layer),
            backgroundColor: try color(.layerBackgroundColor, of: layer),
            borderColor: try color(.layerBorderColor, of: layer),
            shadowColor: try color(.layerShadowColor, of: layer))
    }

    func cellFacet(of object: AnyObject) -> CellFacet {
        do {
            return try cell(object)
        } catch {
            return .raisedGetter
        }
    }

    private func cell(_ object: AnyObject) throws -> CellFacet {
        let cell: AnyObject?
        if ReconNative.isKind(object, of: .control) {
            cell = try related(.controlCell, of: object)
        } else if ReconNative.kind(of: object) == .cell {
            cell = object
        } else {
            return .unsupported
        }
        guard let cell else { return .absent }
        return CellFacet(
            handle: Reference(reference(cell), absence: .unavailable),
            class: bounded(ReconNative.className(of: cell)),
            cellType: try integer(.cellType, of: cell), state: try integer(.cellState, of: cell),
            enabled: try flag(.cellEnabled, of: cell), editable: try flag(.cellEditable, of: cell),
            font: try value(ReconNative.carrierFont(of: cell)).map { FontDescriptor($0, self) }
                ?? .absent)
    }
}
