import SwiftSyntax

/// What `@ReconAction` and `@ReconExports` both derive from one annotated function.
struct ActionSignature {
    struct Parameter {
        let key: String
        let label: String?
        let wrappedType: String
        let optional: Bool
        let defaultValue: String?
        let isContext: Bool
    }

    let name: String
    let descriptorName: String
    let parameters: [Parameter]
    let isAsync: Bool
    let isThrowing: Bool
    let returnType: String?
    let mainActor: Bool

    /// Validates a function as an action. `enclosingMainActor` is the type's own isolation.
    static func analyze(_ function: FunctionDeclSyntax, enclosingMainActor: Bool)
        -> (ActionSignature?, [Problem])
    {
        var problems: [Problem] = []
        let name = function.name.bareName
        if !isLowerCamelCaseASCII(name) {
            problems.append(
                Problem(
                    function.name, "invalid-name",
                    "export name '\(name)' must be lowerCamelCase ASCII letters and digits; "
                        + "rename the function rather than relying on a rewritten export name"))
        }
        if !function.modifiers.contains("static") {
            problems.append(
                Problem(
                    function.name, "not-static",
                    "@ReconAction functions must be static; an action is a function, not a method "
                        + "of a provider instance"))
        }
        if let generics = function.genericParameterClause {
            problems.append(
                Problem(generics, "generic", "@ReconAction functions cannot be generic"))
        }
        let unsupportedAttributes = function.attributes.compactMap { $0.as(AttributeSyntax.self) }
            .filter {
                !["ReconAction", "MainActor", "available", "discardableResult"].contains(
                    $0.attributeName.trimmedDescription)
            }
        for attribute in unsupportedAttributes {
            problems.append(
                Problem(
                    attribute, "attribute",
                    "@\(attribute.attributeName.trimmedDescription) is not supported on an action; "
                        + "actions are nonisolated or @MainActor"))
        }

        var parameters: [Parameter] = []
        for parameter in function.signature.parameterClause.parameters {
            if parameter.ellipsis != nil {
                problems.append(
                    Problem(parameter, "variadic", "variadic action parameters are not supported"))
                continue
            }
            if let attributed = parameter.type.as(AttributedTypeSyntax.self),
                !attributed.specifiers.isEmpty || !attributed.attributes.isEmpty
            {
                problems.append(
                    Problem(
                        parameter.type, "parameter-type",
                        "action parameters must be plain Decodable values"))
                continue
            }
            let label = parameter.firstName.text == "_" ? nil : parameter.firstName.bareName
            let isContext = parameter.type.trimmedDescription == "ReconContext"
            let key = label ?? (parameter.secondName?.bareName ?? "_")
            let (wrapped, optional) = unwrapOptional(parameter.type)
            parameters.append(
                Parameter(
                    key: key, label: label, wrappedType: wrapped, optional: optional,
                    defaultValue: parameter.defaultValue?.value.trimmedDescription,
                    isContext: isContext))
        }
        let keys = parameters.filter { !$0.isContext }.map(\.key)
        if Set(keys).count != keys.count {
            problems.append(
                Problem(
                    function.signature.parameterClause, "duplicate-key",
                    "two parameters would share one payload member name"))
        }
        if parameters.filter(\.isContext).count > 1 {
            problems.append(
                Problem(
                    function.signature.parameterClause, "context",
                    "an action takes at most one ReconContext"))
        }

        let isolatedByAttribute = function.attributes.contains("MainActor")
        let mainActor =
            isolatedByAttribute || (enclosingMainActor && !function.modifiers.contains("nonisolated"))
        guard problems.isEmpty else { return (nil, problems) }
        let signature = ActionSignature(
            name: name,
            descriptorName: "_reconAction_\(name)",
            parameters: parameters,
            isAsync: function.signature.effectSpecifiers?.asyncSpecifier != nil,
            isThrowing: function.signature.effectSpecifiers?.throwsClause != nil,
            returnType: function.signature.returnClause.map { $0.type.trimmedDescription }
                .flatMap { $0 == "Void" || $0 == "()" ? nil : $0 },
            mainActor: mainActor)
        return (signature, [])
    }

    var inputs: [(index: Int, parameter: Parameter)] {
        parameters.filter { !$0.isContext }.enumerated().map { ($0.offset, $0.element) }
    }

    /// The input payload's Decodable form, a generated member rather than a local type: Swift 6.4
    /// (swiftlang-6.4.0.34.1) emits a broken LLVM module for a synthesized conformance on a type
    /// local to a macro-generated accessor.
    func inputDeclaration() -> String? {
        guard !inputs.isEmpty else { return nil }
        var lines = ["struct \(inputName): Decodable {"]
        for (index, parameter) in inputs {
            let memberType =
                parameter.optional || parameter.defaultValue != nil
                ? "\(parameter.wrappedType)?" : parameter.wrappedType
            lines.append("    let _\(index): \(memberType)")
        }
        let cases = inputs.map { "_\($0.index) = \(swiftStringLiteral($0.parameter.key))" }
        lines.append("    enum CodingKeys: String, CodingKey { case \(cases.joined(separator: ", ")) }")
        lines.append("}")
        return lines.joined(separator: "\n")
    }

    private var inputName: String { "_ReconInput_\(name)" }

    /// The generated member: a descriptor whose adapter decodes, invokes, and encodes.
    func descriptorDeclaration() -> String {
        var lines: [String] = []
        lines.append("nonisolated static var \(descriptorName): ReconActionDescriptor {")
        let properties = inputs.map { _, parameter in
            let required = !parameter.optional && parameter.defaultValue == nil
            return "ReconSchemaProperty(\(swiftStringLiteral(parameter.key)), "
                + "ReconSchemaNode.of((\(parameter.wrappedType)).self), required: \(required))"
        }
        let output = returnType.map { "ReconSchemaNode.of((\($0)).self)" } ?? ".object([])"
        lines.append("    return ReconActionDescriptor(")
        lines.append("        name: \(swiftStringLiteral(name)),")
        lines.append("        input: .object([\(properties.joined(separator: ", "))]),")
        lines.append("        output: \(output),")
        lines.append("        invocation: .\(invocationCase) { payload, context in")
        if !inputs.isEmpty {
            lines.append("            let input = try ReconCodec.decode(\(inputName).self, from: payload)")
        }
        let performer = isAsync ? "try await ReconCodec.performAsync" : "try ReconCodec.perform"
        let effects = (isThrowing ? "try " : "") + (isAsync ? "await " : "")
        let call = "\(performer) { \(effects)Self.\(callExpression) }"
        if returnType == nil {
            lines.append("            \(call)")
            lines.append("            return ReconCodec.encodeEmpty()")
        } else {
            lines.append("            let result = \(call)")
            lines.append("            return try ReconCodec.encode(result)")
        }
        lines.append("        }")
        lines.append("    )")
        lines.append("}")
        return lines.joined(separator: "\n")
    }

    private var invocationCase: String {
        switch (mainActor, isAsync) {
        case (false, false): "nonisolated"
        case (false, true): "nonisolatedAsync"
        case (true, false): "mainActor"
        case (true, true): "mainActorAsync"
        }
    }

    private var callExpression: String {
        var index = 0
        let arguments = parameters.map { parameter -> String in
            let value: String
            if parameter.isContext {
                value = "context"
            } else {
                let member = "input._\(index)"
                value = parameter.defaultValue.map { "\(member) ?? (\($0))" } ?? member
                index += 1
            }
            return parameter.label.map { "\($0): \(value)" } ?? value
        }
        return "\(name)(\(arguments.joined(separator: ", ")))"
    }
}
