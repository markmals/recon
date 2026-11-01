import SwiftCompilerPlugin
import SwiftDiagnostics
import SwiftSyntax
import SwiftSyntaxMacros

@main
struct ReconExtensionMacrosPlugin: CompilerPlugin {
    let providingMacros: [Macro.Type] = [
        ReconActionMacro.self, ReconExportsMacro.self, ReconModelMacro.self,
    ]
}

struct ReconMessage: DiagnosticMessage, NoteMessage {
    let message: String
    let id: String
    var severity: DiagnosticSeverity = .error

    var diagnosticID: MessageID { MessageID(domain: "ReconExtension", id: id) }
    var noteID: MessageID { diagnosticID }
}

struct Problem {
    let node: Syntax
    let message: ReconMessage

    init(_ node: some SyntaxProtocol, _ id: String, _ message: String) {
        self.node = Syntax(node)
        self.message = ReconMessage(message: message, id: id)
    }

    var diagnostic: Diagnostic { Diagnostic(node: node, message: message) }
}

extension AttributeListSyntax {
    func contains(_ name: String) -> Bool {
        contains { element in
            guard let attribute = element.as(AttributeSyntax.self) else { return false }
            let spelled = attribute.attributeName.trimmedDescription
            return spelled == name || spelled.hasSuffix(".\(name)")
        }
    }
}

extension DeclModifierListSyntax {
    func contains(_ keyword: String) -> Bool { contains { $0.name.text == keyword } }

    /// The access modifier generated public API must mirror, with its trailing space.
    var accessPrefix: String {
        for keyword in ["public", "package"] where contains(keyword) { return "\(keyword) " }
        return ""
    }
}

extension TokenSyntax {
    /// The identifier without backticks.
    var bareName: String { identifier?.name ?? text }
}

func isLowerCamelCaseASCII(_ name: String) -> Bool {
    guard let first = name.unicodeScalars.first, ("a"..."z").contains(first) else { return false }
    return name.unicodeScalars.allSatisfy {
        ("a"..."z").contains($0) || ("A"..."Z").contains($0) || ("0"..."9").contains($0)
    }
}

func swiftStringLiteral(_ text: String) -> String {
    var escaped = "\""
    for scalar in text.unicodeScalars {
        switch scalar {
        case "\"": escaped += "\\\""
        case "\\": escaped += "\\\\"
        case "\n": escaped += "\\n"
        default: escaped.unicodeScalars.append(scalar)
        }
    }
    return escaped + "\""
}

/// Splits `T?`, `T!`, and `Optional<T>` into their wrapped type.
func unwrapOptional(_ type: TypeSyntax) -> (wrapped: String, optional: Bool) {
    if let optional = type.as(OptionalTypeSyntax.self) {
        return (optional.wrappedType.trimmedDescription, true)
    }
    if let unwrapped = type.as(ImplicitlyUnwrappedOptionalTypeSyntax.self) {
        return (unwrapped.wrappedType.trimmedDescription, true)
    }
    if let named = type.as(IdentifierTypeSyntax.self), named.name.text == "Optional",
        let arguments = named.genericArgumentClause?.arguments, arguments.count == 1,
        let argument = arguments.first
    {
        return (argument.argument.trimmedDescription, true)
    }
    return (type.trimmedDescription, false)
}
