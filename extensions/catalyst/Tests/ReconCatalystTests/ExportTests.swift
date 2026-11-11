import Foundation
import ReconExtensionTesting
import ReconNativeSupport
import Testing

@testable import ReconCatalyst

/// The extension as the helper assembles and the runtime invokes it: one action, one input, and
/// the refusal a target that is not Mac Catalyst gets.
extension CatalystInteraction {
    @Suite
    struct Exports {
        let host: ReconTestHost

        init() throws {
            if let open = ReconNativeSession.current {
                ReconNativeSession.close(identifier: open.identifier)
            }
            ReconNativeSession.open(
                identifier: "catalyst-export-session", helperBuild: "test-build",
                protocolVersion: 2)
            host = try ReconTestHost(CatalystInteractor.reconExtension)
        }

        @Test("The extension exports make-window-key and nothing else")
        func exportsOneAction() {
            let descriptor = CatalystInteractor.reconExtension
            #expect(descriptor.identifier == "catalyst")
            #expect(descriptor.actions.map(\.name) == ["makeWindowKey"])
        }

        @Test("The action publishes one required integer window id")
        func publishesItsInput() throws {
            let action = try #require(CatalystInteractor.reconExtension.actions.first)
            guard case .object(let properties) = action.input else {
                Issue.record("the input schema is not an object")
                return
            }
            #expect(properties.map(\.name) == ["windowId"])
            for property in properties {
                #expect(property.required)
            }
            #expect(properties.first?.schema == Int.reconSchema)
        }

        @Test("A window id of another type is refused before the target is reached")
        func refusesAMalformedWindowId() throws {
            let outcome = try host.invoke("makeWindowKey", #"{"windowId": "42"}"#)
            #expect(outcome.failure?.code == "INVALID_ARGUMENT")
        }

        @Test("A request with no window id is refused")
        func refusesAMissingWindowId() throws {
            let outcome = try host.invoke("makeWindowKey", "{}")
            #expect(outcome.failure?.code == "INVALID_ARGUMENT")
        }

        /// The test process links AppKit and no UIKit, which is what the helper is inside an
        /// AppKit target: the Objective-C runtime resolves no `UIApplication`, and the operation
        /// refuses rather than messaging something else.
        @Test("A target that is not Mac Catalyst is refused, not messaged")
        func refusesANonCatalystTarget() throws {
            let outcome = try host.invoke("makeWindowKey", #"{"windowId": 42}"#)
            let failure = try #require(outcome.failure)
            #expect(failure.code == "INSPECTION_UNAVAILABLE")
            #expect(failure.message.contains("not Mac Catalyst"))
        }
    }
}
