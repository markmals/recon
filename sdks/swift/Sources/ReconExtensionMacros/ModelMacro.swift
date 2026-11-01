import SwiftDiagnostics
import SwiftSyntax
import SwiftSyntaxBuilder
import SwiftSyntaxMacros

/// Derives a schema from the syntax of a synthesized Codable model: its stored properties, their
/// optionality, and an in-body `CodingKeys`. Anything that synthesis would not produce is refused.
public struct ReconModelMacro: ExtensionMacro {
    public static func expansion(
        of node: AttributeSyntax,
        attachedTo declaration: some DeclGroupSyntax,
        providingExtensionsOf type: some TypeSyntaxProtocol,
        conformingTo protocols: [TypeSyntax],
        in context: some MacroExpansionContext
    ) throws -> [ExtensionDeclSyntax] {
        let result: (schema: String?, problems: [Problem])
        if let structure = declaration.as(StructDeclSyntax.self) {
            result = structSchema(structure)
        } else if let enumeration = declaration.as(EnumDeclSyntax.self) {
            result = enumSchema(enumeration)
        } else {
            result = (
                nil,
                [
                    Problem(
                        node, "model-kind",
                        "@ReconModel supports structs and String-backed enums; "
                            + "conform other types to ReconSchema explicitly")
                ]
            )
        }
        result.problems.forEach { context.diagnose($0.diagnostic) }
        guard let schema = result.schema else { return [] }
        let access = declaration.modifiers.accessPrefix
        return [
            try ExtensionDeclSyntax(
                """
                extension \(type.trimmed): ReconSchema {
                    \(raw: access)nonisolated static var reconSchema: ReconSchemaNode { \(raw: schema) }
                }
                """)
        ]
    }

    private static func structSchema(_ structure: StructDeclSyntax) -> (String?, [Problem]) {
        var problems: [Problem] = []
        if let generics = structure.genericParameterClause {
            problems.append(Problem(generics, "model-generic", "generic models need an explicit ReconSchema"))
        }
        problems += customCodecProblems(structure.memberBlock)
        let keys = codingKeys(in: structure.memberBlock, problems: &problems)

        var properties: [String] = []
        for member in structure.memberBlock.members {
            guard let variable = member.decl.as(VariableDeclSyntax.self),
                !variable.modifiers.contains("static"), !variable.modifiers.contains("class")
            else { continue }
            if variable.modifiers.contains("lazy") || !variable.attributes.isEmpty {
                problems.append(
                    Problem(
                        variable, "model-property",
                        "lazy and attributed (wrapped) properties change synthesized coding; "
                            + "declare an explicit ReconSchema"))
                continue
            }
            for binding in variable.bindings where isStored(binding) {
                guard let name = binding.pattern.as(IdentifierPatternSyntax.self)?.identifier.bareName
                else { continue }
                let key: String
                if let keys {
                    guard let renamed = keys[name] else { continue }  // Excluded from coding.
                    key = renamed
                } else {
                    key = name
                }
                if variable.bindingSpecifier.text == "let", binding.initializer != nil {
                    problems.append(
                        Problem(
                            binding, "model-constant",
                            "'\(name)' is encoded but never decoded, so one schema cannot describe "
                                + "both directions; make it 'var' or exclude it from CodingKeys"))
                    continue
                }
                guard let type = binding.typeAnnotation?.type else {
                    problems.append(
                        Problem(
                            binding, "model-untyped",
                            "add an explicit type annotation to '\(name)' so its schema is known"))
                    continue
                }
                let (wrapped, optional) = unwrapOptional(type)
                properties.append(
                    "ReconSchemaProperty(\(swiftStringLiteral(key)), "
                        + "ReconSchemaNode.of((\(wrapped)).self), required: \(!optional))")
            }
        }
        guard problems.isEmpty else { return (nil, problems) }
        return (".object([\(properties.joined(separator: ", "))])", [])
    }

    private static func enumSchema(_ enumeration: EnumDeclSyntax) -> (String?, [Problem]) {
        var problems = customCodecProblems(enumeration.memberBlock)
        let rawType = enumeration.inheritanceClause?.inheritedTypes.first?.type.trimmedDescription
        guard rawType == "String" else {
            return (
                nil,
                [
                    Problem(
                        enumeration.name, "model-enum",
                        "@ReconModel enums must be String-backed without associated values; "
                            + "conform other enums to ReconSchema explicitly")
                ]
            )
        }
        var values: [String] = []
        for member in enumeration.memberBlock.members {
            guard let cases = member.decl.as(EnumCaseDeclSyntax.self) else { continue }
            for element in cases.elements {
                if element.parameterClause != nil {
                    problems.append(
                        Problem(element, "model-enum", "associated values need an explicit ReconSchema"))
                }
                let raw = element.rawValue?.value.as(StringLiteralExprSyntax.self)?
                    .representedLiteralValue
                values.append(swiftStringLiteral(raw ?? element.name.bareName))
            }
        }
        guard problems.isEmpty else { return (nil, problems) }
        return (".string(enumeration: [\(values.joined(separator: ", "))])", [])
    }

    /// Custom `init(from:)`/`encode(to:)` can produce any shape; the schema must be stated.
    private static func customCodecProblems(_ members: MemberBlockSyntax) -> [Problem] {
        members.members.compactMap { member -> Problem? in
            let decl = member.decl
            let custom =
                decl.as(InitializerDeclSyntax.self)?.signature.parameterClause.parameters.first?
                .firstName.text == "from"
                || (decl.as(FunctionDeclSyntax.self).map {
                    $0.name.text == "encode"
                        && $0.signature.parameterClause.parameters.first?.firstName.text == "to"
                } ?? false)
            guard custom else { return nil }
            return Problem(
                decl, "custom-codec",
                "a custom Codable implementation can change the serialized shape; remove "
                    + "@ReconModel and conform to ReconSchema with the schema it actually produces")
        }
    }

    private static func codingKeys(in members: MemberBlockSyntax, problems: inout [Problem])
        -> [String: String]?
    {
        guard
            let keys = members.members.lazy.compactMap({ $0.decl.as(EnumDeclSyntax.self) })
                .first(where: { $0.name.text == "CodingKeys" })
        else { return nil }
        var mapping: [String: String] = [:]
        for member in keys.memberBlock.members {
            guard let cases = member.decl.as(EnumCaseDeclSyntax.self) else { continue }
            for element in cases.elements {
                if let value = element.rawValue?.value {
                    guard let literal = value.as(StringLiteralExprSyntax.self)?.representedLiteralValue
                    else {
                        problems.append(
                            Problem(value, "model-keys", "CodingKeys raw values must be string literals"))
                        continue
                    }
                    mapping[element.name.bareName] = literal
                } else {
                    mapping[element.name.bareName] = element.name.bareName
                }
            }
        }
        return mapping
    }

    private static func isStored(_ binding: PatternBindingSyntax) -> Bool {
        guard let accessors = binding.accessorBlock?.accessors else { return true }
        guard case .accessors(let list) = accessors else { return false }
        return list.allSatisfy { ["willSet", "didSet"].contains($0.accessorSpecifier.text) }
    }
}
