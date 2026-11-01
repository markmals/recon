import SwiftDiagnostics
import SwiftSyntax
import SwiftSyntaxMacros

/// Marks an action and reports why it could not be exported. It introduces no declarations, so it
/// can diagnose a misplaced use anywhere, including at global scope; `@ReconExports` generates.
public struct ReconActionMacro: PeerMacro {
    public static func expansion(
        of node: AttributeSyntax,
        providingPeersOf declaration: some DeclSyntaxProtocol,
        in context: some MacroExpansionContext
    ) throws -> [DeclSyntax] {
        guard let function = declaration.as(FunctionDeclSyntax.self) else {
            context.diagnose(
                Problem(node, "not-function", "@ReconAction applies only to functions").diagnostic)
            return []
        }
        if let problem = placementProblem(of: function, in: context.lexicalContext) {
            context.diagnose(problem.diagnostic)
            return []
        }
        let enclosing = context.lexicalContext.first?.asProtocol(WithAttributesSyntax.self)
        let (_, problems) = ActionSignature.analyze(
            function, enclosingMainActor: enclosing?.attributes.contains("MainActor") ?? false)
        problems.forEach { context.diagnose($0.diagnostic) }
        return []
    }

    /// An action that `@ReconExports` cannot see would silently not be exported; refuse it.
    private static func placementProblem(of function: FunctionDeclSyntax, in lexicalContext: [Syntax])
        -> Problem?
    {
        guard let enclosing = lexicalContext.first else {
            return Problem(
                function.name, "free-function",
                "@ReconAction must be a static function in the body of a @ReconExports type; "
                    + "a free function cannot be collected without restating its name")
        }
        if enclosing.is(ExtensionDeclSyntax.self) {
            return Problem(
                function.name, "in-extension",
                "@ReconAction must be declared in the @ReconExports type's own body; "
                    + "actions in an extension are not collected")
        }
        let attributes = enclosing.asProtocol(WithAttributesSyntax.self)?.attributes
        if attributes?.contains("ReconExports") != true {
            return Problem(
                function.name, "no-exports",
                "the enclosing type is not annotated @ReconExports, so this action would not be "
                    + "exported")
        }
        return nil
    }
}

/// Generates every action's input form and descriptor, then assembles the extension.
public struct ReconExportsMacro: MemberMacro {
    public static func expansion(
        of node: AttributeSyntax,
        providingMembersOf declaration: some DeclGroupSyntax,
        conformingTo protocols: [TypeSyntax],
        in context: some MacroExpansionContext
    ) throws -> [DeclSyntax] {
        guard declaration.is(EnumDeclSyntax.self) || declaration.is(StructDeclSyntax.self) else {
            context.diagnose(
                Problem(
                    node, "namespace-kind",
                    "@ReconExports applies to an enum or struct namespace, not a provider object"
                ).diagnostic)
            return []
        }
        guard let identifier = extensionIdentifier(node, in: context) else { return [] }
        let mainActor = declaration.attributes.contains("MainActor")

        var first: [String: FunctionDeclSyntax] = [:]
        var generated: [String] = []
        var descriptors: [String] = []
        for member in declaration.memberBlock.members {
            guard let function = member.decl.as(FunctionDeclSyntax.self),
                function.attributes.contains("ReconAction")
            else { continue }
            let (signature, _) = ActionSignature.analyze(function, enclosingMainActor: mainActor)
            guard let signature else { continue }  // @ReconAction reports its own problems.
            if let earlier = first[signature.name] {
                context.diagnose(duplicate(signature.name, in: identifier, at: function, earlier: earlier))
                continue
            }
            first[signature.name] = function
            generated += [signature.inputDeclaration(), signature.descriptorDeclaration()].compactMap { $0 }
            descriptors.append("Self.\(signature.descriptorName)")
        }
        let access = declaration.modifiers.accessPrefix
        let assembly = """
            \(access)nonisolated static var reconExtension: ReconExtensionDescriptor {
                ReconExtensionDescriptor(identifier: \(swiftStringLiteral(identifier)), actions: [\(descriptors.joined(separator: ", "))])
            }
            """
        return (generated + [assembly]).map { DeclSyntax(stringLiteral: $0) }
    }

    private static func duplicate(
        _ name: String, in identifier: String, at function: FunctionDeclSyntax,
        earlier: FunctionDeclSyntax
    ) -> Diagnostic {
        Diagnostic(
            node: function.name,
            message: ReconMessage(
                message: "duplicate export '\(name)' in extension '\(identifier)': annotated "
                    + "overloads and repeated names cannot share one action name",
                id: "duplicate-action"),
            notes: [
                Note(
                    node: Syntax(earlier.name),
                    message: ReconMessage(
                        message: "'\(name)' is first exported here", id: "duplicate-action-first"))
            ])
    }

    private static func extensionIdentifier(
        _ node: AttributeSyntax, in context: some MacroExpansionContext
    ) -> String? {
        guard let argument = node.arguments?.as(LabeledExprListSyntax.self)?.first?.expression,
            let literal = argument.as(StringLiteralExprSyntax.self),
            let identifier = literal.representedLiteralValue
        else {
            context.diagnose(
                Problem(
                    node, "identifier-literal",
                    "@ReconExports needs the extension identifier as a plain string literal"
                ).diagnostic)
            return nil
        }
        let valid =
            identifier.first.map { ("a"..."z").contains($0) } == true
            && identifier.allSatisfy { ("a"..."z").contains($0) || ("0"..."9").contains($0) || $0 == "-" }
        guard valid else {
            context.diagnose(
                Problem(
                    literal, "identifier-syntax",
                    "extension identifier '\(identifier)' must be lowercase ASCII letters, digits, "
                        + "and hyphens, starting with a letter"
                ).diagnostic)
            return nil
        }
        return identifier
    }
}
