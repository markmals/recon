import AppKit
import Foundation

/// One colour as the object reported it, resolved into device RGB where that conversion exists.
public struct ReconColorFacts: Sendable, Equatable {
    public let colorSpace: String
    public let catalog: String?
    public let name: String?
    public let converted: Bool
    public let red: Double
    public let green: Double
    public let blue: Double
    public let alpha: Double
}

/// One font as its own descriptor reports it.
public struct ReconFontFacts: Sendable, Equatable {
    public let family: String
    public let postScriptName: String
    public let pointSize: Double
    public let symbolicTraits: Int
    public let hasWeight: Bool
    public let weight: Double
    public let descriptorName: String
}

/// One longest-effective font run of an attributed string.
public struct ReconFontRun {
    public let location: Int
    public let length: Int
    public let font: ReconFontFacts?
}

/// One layout constraint. `firstItem` and `secondItem` are live objects the caller turns into
/// handles; nothing else about them is read here.
public struct ReconConstraintFacts {
    public let firstItem: AnyObject?
    public let secondItem: AnyObject?
    public let firstAttribute: Int
    public let secondAttribute: Int
    public let relation: Int
    public let multiplier: Double
    public let constant: Double
    public let priority: Double
    public let active: Bool
    public let identifier: String?
}

/// One WindowServer window the inventory reports, whoever owns it.
public struct ReconWindowServerEntry: Sendable, Equatable {
    public let number: Int
    public let ownerPid: Int
    public let onscreen: Bool
}

/// Reads whose value is several messages' worth of facts. A getter that raises anywhere in one
/// of them fails the whole read, because a half-read colour or font is not a fact.
enum Facts {
    static func color(of color: AnyObject) -> ReconRead<ReconColorFacts> {
        var reads = ReadSequence()
        let space = reads.optional(Message.object(Fields.colorSpace, to: color))
        let spaceName = space.flatMap { reads.optional(Message.text(Fields.localizedName, to: $0)) }
        let catalog = reads.optional(Message.text(Fields.catalogNameComponent, to: color))
        let name = reads.optional(Message.text(Fields.colorNameComponent, to: color))
        let converted = reads.optional(convertedToDeviceRGB(color))
        let components = converted.map { componentsOf($0, &reads) }
        guard !reads.failed else { return .failed }
        return .value(
            ReconColorFacts(
                colorSpace: spaceName ?? "unknown", catalog: catalog, name: name,
                converted: components != nil, red: components?.red ?? 0,
                green: components?.green ?? 0, blue: components?.blue ?? 0,
                alpha: components?.alpha ?? 0))
    }

    private static func convertedToDeviceRGB(_ color: AnyObject) -> ReconRead<AnyObject> {
        switch Message.object(Fields.deviceRGBColorSpace, to: NSColorSpace.self) {
        case .value(let space):
            Message.object(Fields.colorUsingColorSpace, to: color, with: space)
        case .unavailable: .unavailable
        case .failed: .failed
        }
    }

    private static func componentsOf(_ color: AnyObject, _ reads: inout ReadSequence)
        -> (red: Double, green: Double, blue: Double, alpha: Double)
    {
        (
            reads.optional(Message.double(Fields.redComponent, to: color)) ?? 0,
            reads.optional(Message.double(Fields.greenComponent, to: color)) ?? 0,
            reads.optional(Message.double(Fields.blueComponent, to: color)) ?? 0,
            reads.optional(Message.double(Fields.alphaComponent, to: color)) ?? 0
        )
    }

    static func font(of font: AnyObject) -> ReconRead<ReconFontFacts> {
        var reads = ReadSequence()
        let family = reads.optional(Message.text(Fields.familyName, to: font))
        let postScript = reads.optional(Message.text(Fields.fontName, to: font))
        let size = reads.optional(Message.double(Fields.pointSize, to: font))
        let descriptor = reads.optional(Message.object(Fields.fontDescriptor, to: font))
        let traits = descriptor.flatMap { reads.optional(Message.integer(Fields.symbolicTraits, to: $0)) }
        let descriptorName = descriptor.flatMap {
            reads.optional(Message.text(Fields.postscriptName, to: $0))
        }
        let weight = descriptor.flatMap { weightOf($0, &reads) }
        guard !reads.failed else { return .failed }
        return .value(
            ReconFontFacts(
                family: family ?? "", postScriptName: postScript ?? "", pointSize: size ?? 0,
                symbolicTraits: traits ?? 0, hasWeight: weight != nil, weight: weight ?? 0,
                descriptorName: descriptorName ?? ""))
    }

    private static func weightOf(_ descriptor: AnyObject, _ reads: inout ReadSequence) -> Double? {
        let key = NSFontDescriptor.AttributeName.traits.rawValue as NSString
        guard
            let traits = reads.optional(
                Message.object(Fields.objectForKey, to: descriptor, with: key))
        else { return nil }
        let weightKey = NSFontDescriptor.TraitKey.weight.rawValue as NSString
        guard let value = reads.optional(Message.object(Fields.objectForKey, to: traits, with: weightKey))
        else { return nil }
        return reads.optional(Message.double(Fields.doubleValue, to: value))
    }

    static func constraint(_ constraint: AnyObject) -> ReconRead<ReconConstraintFacts> {
        var reads = ReadSequence()
        let first = reads.optional(Message.object(Fields.firstItem, to: constraint))
        let second = reads.optional(Message.object(Fields.secondItem, to: constraint))
        let firstAttribute = reads.optional(Message.integer(Fields.firstAttribute, to: constraint))
        let secondAttribute = reads.optional(
            Message.integer(Fields.secondAttribute, to: constraint))
        let relation = reads.optional(Message.integer(Fields.relation, to: constraint))
        let multiplier = reads.optional(Message.double(Fields.multiplier, to: constraint))
        let constant = reads.optional(Message.double(Fields.constant, to: constraint))
        let priority = reads.optional(Message.float(Fields.priority, to: constraint))
        let active = reads.optional(Message.flag(Fields.isActive, to: constraint))
        let identifier = reads.optional(Message.text(Fields.identifier, to: constraint))
        guard !reads.failed else { return .failed }
        return .value(
            ReconConstraintFacts(
                firstItem: first, secondItem: second, firstAttribute: firstAttribute ?? 0,
                secondAttribute: secondAttribute ?? 0, relation: relation ?? 0,
                multiplier: multiplier ?? 0, constant: constant ?? 0, priority: priority ?? 0,
                active: active ?? false, identifier: identifier))
    }
}

/// Several reads that make one fact: an absent value is still a fact, a failed one is not.
struct ReadSequence {
    private(set) var failed = false

    mutating func optional<Value>(_ read: ReconRead<Value>) -> Value? {
        switch read {
        case .value(let value): return value
        case .unavailable: return nil
        case .failed:
            failed = true
            return nil
        }
    }
}
