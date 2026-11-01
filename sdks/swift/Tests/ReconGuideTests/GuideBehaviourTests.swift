import AppKit
import Foundation
import ReconExtensionTesting
import Testing

/// The two tests the guide prints, verbatim.
@Suite(.serialized)
struct GuideTests {
    @Test func greetsByName() throws {
        let host = try ReconTestHost(Demo.reconExtension)
        let greeting = try host.invoke("greeting", #"{"name":"Ada","excited":true}"#).decode(
            Greeting.self)
        #expect(greeting == Greeting(message: "Hello, Ada", excited: true))
    }

    @Test func rejectsAnEmptyName() throws {
        let host = try ReconTestHost(Demo.reconExtension)
        #expect(try host.invoke("greeting", #"{"name":""}"#).failure?.code == "INVALID_ARGUMENT")
    }
}

/// What the guide's prose says each example answers.
@Suite(.serialized)
struct GuideClaims {
    @Test func anAbsentOptionalIsOmittedRatherThanNull() throws {
        let host = try ReconTestHost(Demo.reconExtension)
        guard case .success(let result) = try host.invoke("greeting", #"{"name":"Ada"}"#) else {
            Issue.record("greeting failed")
            return
        }
        #expect(String(decoding: result, as: UTF8.self) == #"{"message":"Hello, Ada"}"#)
    }

    @Test func thePublishedInputSchemaIsAsPrinted() throws {
        let descriptor = try #require(Demo.reconExtension.actions.first { $0.name == "greeting" })
        #expect(
            descriptor.input.jsonSchema
                == #"{"type":"object","properties":{"name":{"type":"string"},"excited":{"type":"boolean"}},"required":["name"],"additionalProperties":false}"#
        )
    }

    @Test func anUndeclaredMemberIsRejectedBeforeTheFunctionRuns() throws {
        let host = try ReconTestHost(Demo.reconExtension)
        #expect(
            try host.invoke("greeting", #"{"name":"Ada","shout":true}"#).failure?.code
                == "INVALID_ARGUMENT")
    }

    @Test func anArgumentLabelNamesTheInputMember() throws {
        let host = try ReconTestHost(Counter.reconExtension)
        #expect(try host.invoke("count", #"{"upTo":3}"#).decode(Int.self) == 3)
        #expect(try host.invoke("count", #"{"limit":3}"#).failure?.code == "INVALID_ARGUMENT")
    }

    @Test func aNonFiniteResultFailsAtTheEncodingStage() throws {
        let host = try ReconTestHost(Unencodable.reconExtension)
        let failure = try #require(try host.invoke("ratio").failure)
        #expect(failure.code == "INSPECTION_FAILED")
        #expect(failure.stage == "encoding")
    }

    @Test func equalActionNamesInDifferentExtensionsAreFine() throws {
        #expect(throws: Never.self) { try ReconTestHost(Demo.reconExtension, Echo.reconExtension) }
    }

    @Test func twoExtensionsWithOneIdentifierAreRejected() {
        #expect(throws: (any Error).self) {
            try ReconTestHost(Demo.reconExtension, Demo.reconExtension)
        }
    }

    @MainActor
    @Test func nativeAccessorsReportValueUnavailableAndFailure() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 120, height: 80), styleMask: [.titled],
            backing: .buffered, defer: true)
        window.title = "Fixture"
        #expect(title(of: window) == "Fixture")
        #expect(title(of: NSView(frame: .zero)) == nil)
        #expect(title(of: RaisingTitle()) == nil)
    }

    @Test func aCustomCodecPublishesTheShapeItProduces() {
        #expect(Point.reconSchema.jsonSchema == #"{"type":"array","items":{"type":"number"}}"#)
    }
}

/// A `title` getter that raises, which native support must catch before Swift sees it.
private final class RaisingTitle: NSObject {
    @objc var title: String {
        NSException(name: .genericException, reason: "unreadable", userInfo: nil).raise()
        return ""
    }
}

@ReconExports("unencodable")
private enum Unencodable {
    @ReconAction
    static func ratio() -> Double { .nan }
}

@ReconExports("echo")
private enum Echo {
    @ReconAction
    static func greeting(_ name: String) -> Greeting { Greeting(message: name) }
}
