import Testing

/// What the guide's "Limits and edge cases" promises is refused, refused by the real compiler.
struct Diagnostics {
    @Test(arguments: rejections)
    func rejectedSnippetsExplainThemselves(_ rejection: Rejection) throws {
        let diagnostics = try Compiler.diagnostics(for: rejection.snippet)
        #expect(!diagnostics.isEmpty, "\(rejection.name) compiled")
        #expect(
            diagnostics.contains(rejection.message),
            "\(rejection.name) reported:\n\(diagnostics)")
    }

    @Test func aWellFormedExtensionCompiles() throws {
        let diagnostics = try Compiler.diagnostics(
            for: """
                @ReconModel
                struct Answer: Codable { let text: String }

                @ReconExports("demo")
                enum Demo {
                    @ReconAction
                    static func answer(_ text: String, context: ReconContext) throws -> Answer {
                        try context.checkCancellation()
                        return Answer(text: text)
                    }
                }
                """)
        #expect(diagnostics.isEmpty, "\(diagnostics)")
    }
}

struct Rejection: Sendable, CustomTestStringConvertible {
    let name: String
    let snippet: String
    let message: String

    var testDescription: String { name }
}

private let rejections: [Rejection] = [
    Rejection(
        name: "free function",
        snippet: """
            @ReconAction
            func greeting(_ name: String) -> String { name }
            """,
        message: "@ReconAction must be a static function in the body of a @ReconExports type"),
    Rejection(
        name: "instance method",
        snippet: """
            @ReconExports("demo")
            struct Demo {
                @ReconAction
                func greeting(_ name: String) -> String { name }
            }
            """,
        message: "@ReconAction functions must be static"),
    Rejection(
        name: "action in an extension",
        snippet: """
            @ReconExports("demo")
            enum Demo {}

            extension Demo {
                @ReconAction
                static func greeting(_ name: String) -> String { name }
            }
            """,
        message: "@ReconAction must be declared in the @ReconExports type's own body"),
    Rejection(
        name: "namespace without @ReconExports",
        snippet: """
            enum Demo {
                @ReconAction
                static func greeting(_ name: String) -> String { name }
            }
            """,
        message: "the enclosing type is not annotated @ReconExports"),
    Rejection(
        name: "annotated overloads",
        snippet: """
            @ReconExports("demo")
            enum Demo {
                @ReconAction
                static func greeting(_ name: String) -> String { name }

                @ReconAction
                static func greeting(count: Int) -> String { "\\(count)" }
            }
            """,
        message: "duplicate export 'greeting' in extension 'demo'"),
    Rejection(
        name: "variadic parameter",
        snippet: """
            @ReconExports("demo")
            enum Demo {
                @ReconAction
                static func greeting(names: String...) -> String { names.joined() }
            }
            """,
        message: "variadic action parameters are not supported"),
    Rejection(
        name: "inout parameter",
        snippet: """
            @ReconExports("demo")
            enum Demo {
                @ReconAction
                static func greeting(name: inout String) -> String { name }
            }
            """,
        message: "action parameters must be plain Decodable values"),
    Rejection(
        name: "generic action",
        snippet: """
            @ReconExports("demo")
            enum Demo {
                @ReconAction
                static func greeting<T: Codable>(value: T) -> String { "\\(value)" }
            }
            """,
        message: "@ReconAction functions cannot be generic"),
    Rejection(
        name: "another global actor",
        snippet: """
            @globalActor actor Inspector {
                static let shared = Inspector()
            }

            @ReconExports("demo")
            enum Demo {
                @ReconAction
                @Inspector
                static func greeting(_ name: String) -> String { name }
            }
            """,
        message: "is not supported on an action"),
    Rejection(
        name: "name that is not lowerCamelCase",
        snippet: """
            @ReconExports("demo")
            enum Demo {
                @ReconAction
                static func Greeting(_ name: String) -> String { name }
            }
            """,
        message: "must be lowerCamelCase ASCII letters and digits"),
    Rejection(
        name: "two parameters sharing one member name",
        snippet: """
            @ReconExports("demo")
            enum Demo {
                @ReconAction
                static func greeting(name first: String, name second: String) -> String { first }
            }
            """,
        message: "two parameters would share one payload member name"),
    Rejection(
        name: "two contexts",
        snippet: """
            @ReconExports("demo")
            enum Demo {
                @ReconAction
                static func greeting(a: ReconContext, b: ReconContext) -> String { "" }
            }
            """,
        message: "an action takes at most one ReconContext"),
    Rejection(
        name: "invalid extension identifier",
        snippet: """
            @ReconExports("Demo Extension")
            enum Demo {}
            """,
        message: "must be lowercase ASCII letters, digits, and hyphens, starting with a letter"),
    Rejection(
        name: "namespace that is not an enum or struct",
        snippet: """
            @ReconExports("demo")
            final class Demo {}
            """,
        message: "@ReconExports applies to an enum or struct namespace"),
    Rejection(
        name: "a parameter type without a schema",
        snippet: """
            import Foundation

            @ReconExports("demo")
            enum Demo {
                @ReconAction
                static func at(moment: Date) -> String { "\\(moment)" }
            }
            """,
        message: "requires that 'Date' conform to 'ReconSchema'"),
    Rejection(
        name: "@ReconModel with a custom codec",
        snippet: """
            @ReconModel
            struct Answer: Codable {
                let text: String

                func encode(to encoder: any Encoder) throws {
                    var container = encoder.singleValueContainer()
                    try container.encode(text)
                }
            }
            """,
        message: "a custom Codable implementation can change the serialized shape"),
    Rejection(
        name: "@ReconModel with an encode-only constant",
        snippet: """
            @ReconModel
            struct Answer: Codable {
                let kind: String = "answer"
                let text: String
            }
            """,
        message: "is encoded but never decoded"),
    Rejection(
        name: "@ReconModel with an untyped property",
        snippet: """
            @ReconModel
            struct Answer: Codable {
                var text = "answer"
                var count: Int
            }
            """,
        message: "add an explicit type annotation"),
    Rejection(
        name: "@ReconModel enum with associated values",
        snippet: """
            @ReconModel
            enum Answer: Codable {
                case text(String)
            }
            """,
        message: "@ReconModel enums must be String-backed without associated values"),
    Rejection(
        name: "@ReconModel on a generic type",
        snippet: """
            @ReconModel
            struct Answer<Value: Codable>: Codable {
                let value: Value
            }
            """,
        message: "generic models need an explicit ReconSchema"),
]
