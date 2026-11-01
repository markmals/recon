extension ReconSchemaNode {
    /// JSON Schema text, as the command registry publishes it.
    public var jsonSchema: String {
        switch self {
        case .string(let enumeration?):
            "{\"enum\":[\(enumeration.map(quoted).joined(separator: ","))]}"
        case .string(nil):
            "{\"type\":\"string\"}"
        case .boolean:
            "{\"type\":\"boolean\"}"
        case .integer(let minimum, let maximum):
            "{\"type\":\"integer\",\"minimum\":\(minimum),\"maximum\":\(maximum)}"
        case .number:
            "{\"type\":\"number\"}"
        case .array(let element):
            "{\"type\":\"array\",\"items\":\(element.jsonSchema)}"
        case .dictionary(let value):
            "{\"type\":\"object\",\"additionalProperties\":\(value.jsonSchema)}"
        case .object(let properties):
            objectSchema(properties)
        case .nullable(let wrapped):
            "{\"anyOf\":[\(wrapped.jsonSchema),{\"type\":\"null\"}]}"
        case .oneOf(let alternatives):
            "{\"anyOf\":[\(alternatives.map(\.jsonSchema).joined(separator: ","))]}"
        }
    }

    private func objectSchema(_ properties: [ReconSchemaProperty]) -> String {
        let members = properties.map { "\(quoted($0.name)):\($0.schema.jsonSchema)" }
        let required = properties.filter(\.required).map { quoted($0.name) }
        return "{\"type\":\"object\",\"properties\":{\(members.joined(separator: ","))},"
            + "\"required\":[\(required.joined(separator: ","))],\"additionalProperties\":false}"
    }
}

private func quoted(_ text: String) -> String {
    var escaped = "\""
    for scalar in text.unicodeScalars {
        switch scalar {
        case "\"": escaped += "\\\""
        case "\\": escaped += "\\\\"
        case _ where scalar.value < 0x20: escaped += hexEscape(scalar.value)
        default: escaped.unicodeScalars.append(scalar)
        }
    }
    return escaped + "\""
}

private func hexEscape(_ value: UInt32) -> String {
    let digits = "0123456789abcdef"
    let nibbles = [12, 8, 4, 0].map { shift -> Character in
        let index = Int((value >> UInt32(shift)) & 0xF)
        return digits[digits.index(digits.startIndex, offsetBy: index)]
    }
    return "\\u" + String(nibbles)
}
