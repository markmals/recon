import ReconExtension
import ReconNativeSupport

/// One end of a constraint, by handle. A constraint can also hold something Recon publishes no
/// handles for, which is a kind it does not support rather than a missing object.
@ReconModel
struct ConstraintItem: Codable, Equatable {
    let kind: String?
    let handle: String?
    let status: StatusName?

    static let unsupported = ConstraintItem(kind: nil, handle: nil, status: .unsupported)
    static let absent = ConstraintItem(kind: nil, handle: nil, status: .absent)
}

@ReconModel
struct Constraint: Codable, Equatable {
    let owner: ConstraintItem
    let firstItem: ConstraintItem
    let secondItem: ConstraintItem
    let firstAttribute: Int
    let secondAttribute: Int
    let relation: Int
    let multiplier: Scalar
    let constant: Scalar
    let priority: Scalar
    let active: Bool
    let identifier: TextOrStatus
}

@ReconModel
struct LayoutGuideReference: Codable, Equatable {
    let kind: String
    let handle: Reference
}

/// An intrinsic size, which a view may have none of.
@ReconModel
struct Metric: Codable, Equatable {
    let status: StatusName
    let value: Double?

    /// AppKit's "no intrinsic metric" sentinel, which is the absence of a size rather than one.
    static let noIntrinsicMetric = -1.0
    /// A layout guide contributes no intrinsic size of its own.
    static let unsupported = Metric(status: .unsupported, value: nil)

    private init(status: StatusName, value: Double?) {
        self.status = status
        self.value = value
    }

    init(_ number: Double) {
        if number == Self.noIntrinsicMetric {
            status = .noIntrinsicMetric
            value = nil
        } else {
            status = number.isFinite ? .value : .nonFinite
            value = number.isFinite ? number : nil
        }
    }
}

@ReconModel
struct PriorityPair: Codable, Equatable {
    let horizontal: Scalar?
    let vertical: Scalar?
    let status: StatusName?

    static let unsupported = PriorityPair(horizontal: nil, vertical: nil, status: .unsupported)

    init(horizontal: Scalar?, vertical: Scalar?, status: StatusName?) {
        self.horizontal = horizontal
        self.vertical = vertical
        self.status = status
    }

    init(_ horizontal: Double, _ vertical: Double) {
        self.init(horizontal: Scalar(horizontal), vertical: Scalar(vertical), status: nil)
    }
}

/// The constraints that touch an object, and what it contributes to layout itself.
@ReconModel
struct LayoutFacet: Codable, Equatable {
    var status: StatusName = .value
    var reason: String? = nil
    var constraints: [Constraint]? = nil
    var layoutGuides: [LayoutGuideReference]? = nil
    var intrinsicWidth: Metric? = nil
    var intrinsicHeight: Metric? = nil
    var hugging: PriorityPair? = nil
    var compressionResistance: PriorityPair? = nil

    static let unsupported = LayoutFacet(status: .unsupported)
    static let absent = LayoutFacet(status: .absent)
    static let raisedGetter = LayoutFacet(status: .failed, reason: "target-getter-exception")
}

extension Work {
    func layoutFacet(of object: AnyObject) -> LayoutFacet {
        do {
            return try layout(object)
        } catch {
            return .raisedGetter
        }
    }

    private func layout(_ object: AnyObject) throws -> LayoutFacet {
        let isGuide = ReconNative.kind(of: object) == .layoutGuide
        guard isGuide || ReconNative.kind(of: object) == .view else { return .unsupported }
        guard let view = isGuide ? try related(.guideOwningView, of: object) : object else {
            return .absent
        }
        let constraints = try self.constraints(touching: object, from: view, isGuide: isGuide)
        guard !isGuide else {
            return LayoutFacet(
                constraints: constraints, layoutGuides: [], intrinsicWidth: .unsupported,
                intrinsicHeight: .unsupported, hugging: .unsupported,
                compressionResistance: .unsupported)
        }
        let size = try value(ReconNative.size(.viewIntrinsicContentSize, of: view)) ?? .zero
        return LayoutFacet(
            constraints: constraints, layoutGuides: try layoutGuides(of: view),
            intrinsicWidth: Metric(size.width), intrinsicHeight: Metric(size.height),
            hugging: PriorityPair(
                try number(.viewHuggingHorizontal, of: view) ?? .nan,
                try number(.viewHuggingVertical, of: view) ?? .nan),
            compressionResistance: PriorityPair(
                try number(.viewCompressionHorizontal, of: view) ?? .nan,
                try number(.viewCompressionVertical, of: view) ?? .nan))
    }

    /// Every constraint on the object's own view and its ancestors that names the object or one
    /// of them. A constraint reached from two owners is reported once.
    private func constraints(touching object: AnyObject, from view: AnyObject, isGuide: Bool) throws
        -> [Constraint]
    {
        var collected: [Constraint] = []
        var seen: Set<ObjectIdentifier> = []
        var lineage: Set<ObjectIdentifier> = []
        var owner: AnyObject? = view
        while let current = owner, !expired {
            if !isGuide { lineage.insert(ObjectIdentifier(current)) }
            if collected.count >= limit { break }
            try collect(
                on: current, object: object, isGuide: isGuide, lineage: lineage, seen: &seen,
                into: &collected)
            if visited >= Self.visitLimit || collected.count >= limit { break }
            owner = try related(.viewSuperview, of: current)
        }
        return collected
    }

    private func collect(
        on owner: AnyObject, object: AnyObject, isGuide: Bool, lineage: Set<ObjectIdentifier>,
        seen: inout Set<ObjectIdentifier>, into collected: inout [Constraint]
    ) throws {
        for constraint in try list(.viewConstraints, of: owner) {
            if collected.count >= limit {
                note(truncated: "layout-constraints:limit")
                break
            }
            if !consume() { break }
            guard seen.insert(ObjectIdentifier(constraint)).inserted else { continue }
            guard let facts = try value(ReconNative.constraint(constraint)) else { continue }
            guard
                touches(facts, owner: owner, view: object, isGuide: isGuide, lineage: lineage)
            else { continue }
            collected.append(described(facts, owner: owner))
        }
    }

    private func touches(
        _ facts: ReconConstraintFacts, owner: AnyObject, view: AnyObject, isGuide: Bool,
        lineage: Set<ObjectIdentifier>
    ) -> Bool {
        if !isGuide, owner === view { return true }
        if facts.firstItem === view || facts.secondItem === view { return true }
        guard !isGuide else { return false }
        return facts.firstItem.map { lineage.contains(ObjectIdentifier($0)) } == true
            || facts.secondItem.map { lineage.contains(ObjectIdentifier($0)) } == true
    }

    private func described(_ facts: ReconConstraintFacts, owner: AnyObject) -> Constraint {
        Constraint(
            owner: item(owner), firstItem: item(facts.firstItem),
            secondItem: facts.secondItem.map(item) ?? .absent,
            firstAttribute: facts.firstAttribute, secondAttribute: facts.secondAttribute,
            relation: facts.relation, multiplier: Scalar(facts.multiplier),
            constant: Scalar(facts.constant), priority: Scalar(facts.priority),
            active: facts.active, identifier: TextOrStatus(facts.identifier.map(bounded)))
    }

    private func item(_ object: AnyObject?) -> ConstraintItem {
        guard let object, let kind = ReconNative.kind(of: object).publishedName else {
            return .unsupported
        }
        guard let handle = reference(object) else {
            return ConstraintItem(kind: nil, handle: nil, status: .unavailable)
        }
        return ConstraintItem(kind: kind, handle: handle, status: nil)
    }

    private func layoutGuides(of view: AnyObject) throws -> [LayoutGuideReference] {
        var guides: [LayoutGuideReference] = []
        for guide in try list(.viewLayoutGuides, of: view) {
            if guides.count >= limit {
                note(truncated: "layout-guides:limit")
                break
            }
            if !consume() { break }
            guides.append(
                LayoutGuideReference(
                    kind: "layoutGuide",
                    handle: Reference(reference(guide), absence: .unavailable)))
        }
        return guides
    }
}
