import Foundation
import ReconExtension

/// Checks a parsed JSON value against a published schema, so decoder defaults (ignored unknown
/// keys, `null` treated as absent) cannot widen the contract, and encoders cannot narrow it.
enum SchemaValidator {
    /// The first violation, or nil when the payload conforms.
    static func violation(in payload: Data, against schema: ReconSchemaNode) -> String? {
        guard let value = try? JSONSerialization.jsonObject(with: payload, options: [.fragmentsAllowed])
        else { return "payload is not JSON" }
        return violation(value, schema, path: "$")
    }

    private static func violation(_ value: Any, _ schema: ReconSchemaNode, path: String) -> String? {
        switch schema {
        case .nullable(let wrapped):
            return value is NSNull ? nil : violation(value, wrapped, path: path)
        case .string(let enumeration):
            guard let string = value as? String else { return "\(path) must be a string" }
            if let enumeration, !enumeration.contains(string) {
                return "\(path) must be one of \(enumeration)"
            }
            return nil
        case .boolean:
            return isBoolean(value) ? nil : "\(path) must be a boolean"
        case .number:
            guard let number = value as? NSNumber, !isBoolean(value), number.doubleValue.isFinite
            else { return "\(path) must be a number" }
            return nil
        case .integer(let minimum, let maximum):
            guard let integer = integerValue(value), integer >= minimum, integer <= maximum else {
                return "\(path) must be an integer in \(minimum)...\(maximum)"
            }
            return nil
        case .array(let element):
            guard let array = value as? [Any] else { return "\(path) must be an array" }
            for (index, item) in array.enumerated() {
                if let found = violation(item, element, path: "\(path)[\(index)]") { return found }
            }
            return nil
        case .dictionary(let element):
            guard let object = value as? [String: Any] else { return "\(path) must be an object" }
            for (key, item) in object {
                if let found = violation(item, element, path: "\(path).\(key)") { return found }
            }
            return nil
        case .object(let properties):
            guard let object = value as? [String: Any] else { return "\(path) must be an object" }
            let known = Set(properties.map(\.name))
            if let unknown = object.keys.sorted().first(where: { !known.contains($0) }) {
                return "\(path).\(unknown) is not a published member"
            }
            for property in properties {
                guard let item = object[property.name] else {
                    if property.required { return "\(path).\(property.name) is required" }
                    continue
                }
                if let found = violation(item, property.schema, path: "\(path).\(property.name)") {
                    return found
                }
            }
            return nil
        }
    }

    private static func isBoolean(_ value: Any) -> Bool {
        guard let number = value as? NSNumber else { return false }
        return CFGetTypeID(number) == CFBooleanGetTypeID()
    }

    private static func integerValue(_ value: Any) -> Int128? {
        guard let number = value as? NSNumber, !isBoolean(value) else { return nil }
        if let decimal = number as? NSDecimalNumber {
            return Int128(exactly: decimal.doubleValue)
        }
        switch String(cString: number.objCType) {
        case "d", "f": return Int128(exactly: number.doubleValue)
        case "Q": return Int128(number.uint64Value)
        default: return Int128(number.int64Value)
        }
    }
}
