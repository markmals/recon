import ReconExtension
import ReconNativeSupport

@ReconModel
struct VisualEffect: Codable, Equatable {
    let material: Int
    let blendingMode: Int
    let state: Int
    let emphasized: Bool
}

/// How an object is drawn: its appearance, and the colours it carries itself.
@ReconModel
struct AppearanceFacet: Codable, Equatable {
    var status: StatusName = .value
    var reason: String? = nil
    var effectiveAppearance: TextOrStatus? = nil
    var appearance: TextOrStatus? = nil
    var visualEffect: VisualEffect? = nil
    var textColor: Color? = nil
    var backgroundColor: Color? = nil
    var fillColor: Color? = nil

    static let unsupported = AppearanceFacet(status: .unsupported)
    static let raisedGetter = AppearanceFacet(status: .failed, reason: "target-getter-exception")
}

extension Work {
    func appearanceFacet(of object: AnyObject) -> AppearanceFacet {
        do {
            return try appearance(object)
        } catch {
            return .raisedGetter
        }
    }

    private func appearance(_ object: AnyObject) throws -> AppearanceFacet {
        var facet = AppearanceFacet()
        switch ReconNative.kind(of: object) {
        case .view: try describeViewAppearance(object, into: &facet)
        case .window:
            facet.appearance = TextOrStatus(try text(.windowAppearanceName, of: object))
            facet.backgroundColor = try color(.windowBackgroundColor, of: object)
        case .cell where ReconNative.isKind(object, of: .textFieldCell):
            facet.textColor = try color(.textFieldCellTextColor, of: object)
        case .cell:
            break
        default:
            return .unsupported
        }
        return facet
    }

    private func describeViewAppearance(_ view: AnyObject, into facet: inout AppearanceFacet) throws
    {
        facet.effectiveAppearance = TextOrStatus(
            try text(.viewEffectiveAppearanceName, of: view))
        if ReconNative.isKind(view, of: .visualEffectView) {
            facet.visualEffect = VisualEffect(
                material: try integer(.visualEffectMaterial, of: view) ?? 0,
                blendingMode: try integer(.visualEffectBlendingMode, of: view) ?? 0,
                state: try integer(.visualEffectState, of: view) ?? 0,
                emphasized: try flag(.visualEffectEmphasized, of: view) ?? false)
        }
        if ReconNative.isKind(view, of: .textField) {
            facet.textColor = try color(.textFieldTextColor, of: view)
            facet.backgroundColor = try color(.textFieldBackgroundColor, of: view)
        } else if ReconNative.isKind(view, of: .box) {
            facet.fillColor = try color(.boxFillColor, of: view)
        }
    }
}
