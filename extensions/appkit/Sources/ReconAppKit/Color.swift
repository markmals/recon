import ReconExtension
import ReconNativeSupport

@ReconModel
struct ColorComponents: Codable, Equatable {
    let red: NumberValue
    let green: NumberValue
    let blue: NumberValue
    let alpha: NumberValue
}

/// One colour as the object reports it, resolved into device RGB where that conversion exists.
@ReconModel
struct Color: Codable, Equatable {
    let status: StatusName
    let colorSpace: String?
    let catalog: String?
    let name: String?
    let resolvedSpace: String?
    let components: ColorComponents?
    let conversion: Status?

    static let absent = Color(
        status: .absent, colorSpace: nil, catalog: nil, name: nil, resolvedSpace: nil,
        components: nil, conversion: nil)

    init(
        status: StatusName, colorSpace: String?, catalog: String?, name: String?,
        resolvedSpace: String?, components: ColorComponents?, conversion: Status?
    ) {
        self.status = status
        self.colorSpace = colorSpace
        self.catalog = catalog
        self.name = name
        self.resolvedSpace = resolvedSpace
        self.components = components
        self.conversion = conversion
    }

    init(_ facts: ReconColorFacts, _ work: Work) {
        status = .value
        colorSpace = work.bounded(facts.colorSpace)
        catalog = facts.catalog.map(work.bounded)
        name = facts.name.map(work.bounded)
        resolvedSpace = facts.converted ? "deviceRGB" : nil
        components =
            facts.converted
            ? ColorComponents(
                red: NumberValue(facts.red), green: NumberValue(facts.green),
                blue: NumberValue(facts.blue), alpha: NumberValue(facts.alpha))
            : nil
        conversion = facts.converted ? nil : Status(.unsupported)
    }
}

extension Work {
    /// A colour read, with a raising getter failing the facet that asked for it.
    func color(_ field: ReconColorField, of object: AnyObject) throws -> Color {
        switch ReconNative.color(field, of: object) {
        case .value(let facts): Color(facts, self)
        case .unavailable: .absent
        case .failed: throw TargetGetterRaised()
        }
    }
}
