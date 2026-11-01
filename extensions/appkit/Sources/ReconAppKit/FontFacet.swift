import ReconExtension
import ReconNativeSupport

/// One font as its own descriptor reports it.
@ReconModel
struct FontDescriptor: Codable, Equatable {
    let status: StatusName
    let family: String?
    let postScriptName: String?
    let pointSize: Scalar?
    let symbolicTraits: Int?
    let weight: OptionalScalar?
    let descriptor: String?

    static let absent = FontDescriptor(
        status: .absent, family: nil, postScriptName: nil, pointSize: nil, symbolicTraits: nil,
        weight: nil, descriptor: nil)

    init(
        status: StatusName, family: String?, postScriptName: String?, pointSize: Scalar?,
        symbolicTraits: Int?, weight: OptionalScalar?, descriptor: String?
    ) {
        self.status = status
        self.family = family
        self.postScriptName = postScriptName
        self.pointSize = pointSize
        self.symbolicTraits = symbolicTraits
        self.weight = weight
        self.descriptor = descriptor
    }

    init(_ facts: ReconFontFacts, _ work: Work) {
        status = .value
        family = work.bounded(facts.family)
        postScriptName = work.bounded(facts.postScriptName)
        pointSize = Scalar(facts.pointSize)
        symbolicTraits = Int(facts.symbolicTraits)
        weight = OptionalScalar(facts.hasWeight ? facts.weight : nil)
        descriptor = work.bounded(facts.descriptorName)
    }
}

@ReconModel
struct TextRange: Codable, Equatable {
    let location: Int
    let length: Int
}

@ReconModel
struct FontRun: Codable, Equatable {
    let range: TextRange
    let font: FontDescriptor
    let source: String
}

/// The fonts an object carries: the one it draws with, and the runs of an attributed value.
@ReconModel
struct FontFacet: Codable, Equatable {
    var status: StatusName = .value
    var reason: String? = nil
    var carrier: FontDescriptor? = nil
    var source: String? = nil
    var runs: [FontRun]? = nil

    static let absent = FontFacet(status: .absent)
    static let raisedGetter = FontFacet(status: .failed, reason: "target-getter-exception")
}

extension Work {
    func fontFacet(of object: AnyObject) -> FontFacet {
        do {
            return try font(object)
        } catch {
            return .raisedGetter
        }
    }

    /// The font facet, also used on its own by the `fonts` read.
    func font(_ object: AnyObject) throws -> FontFacet {
        let carrier = try value(ReconNative.carrierFont(of: object)).map { FontDescriptor($0, self) }
        let runs = try attributedRuns(of: object)
        guard carrier != nil || !runs.isEmpty else { return .absent }
        return FontFacet(
            carrier: carrier ?? .absent,
            source: ReconNative.kind(of: object) == .cell ? "cell" : "view", runs: runs)
    }

    /// The longest effective font runs of an attributed value, bounded by how many runs one
    /// observation carries and how many the walk may examine.
    private func attributedRuns(of object: AnyObject) throws -> [FontRun] {
        guard let attributed = try related(.attributedStringValue, of: object) else { return [] }
        let length = try integer(.attributedStringLength, of: attributed) ?? 0
        var runs: [FontRun] = []
        var offset = 0
        var examined = 0
        while offset < length, examined < Self.runLimit, runs.count < limit, consume() {
            examined += 1
            guard let run = try value(ReconNative.fontRun(of: attributed, at: offset)) else {
                break
            }
            if let font = run.font {
                runs.append(
                    FontRun(
                        range: TextRange(location: Int(run.location), length: Int(run.length)),
                        font: FontDescriptor(font, self), source: "attributed-run"))
            }
            offset = Int(run.location + run.length)
        }
        if offset < length, examined == Self.runLimit {
            note(truncated: "attributed-runs:\(Self.runLimit)")
        } else if offset < length, runs.count == limit {
            note(truncated: "attributed-runs:limit")
        }
        return runs
    }

    /// How many runs of one attributed value a read will examine, whatever its own limit.
    static let runLimit = 256
}
