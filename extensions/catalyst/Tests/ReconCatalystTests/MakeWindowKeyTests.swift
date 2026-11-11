import Foundation
import ReconExtension
import Testing

@testable import ReconCatalyst

/// Resolving the selected WindowServer window to one `UIWindow`, making it key, and what is
/// reported on either side of that.
extension CatalystInteraction {
    @Suite
    struct MakeKey {
        @Test("The one UIWindow hosted in the selected window is the one made key")
        func makesTheSelectedWindowKey() throws {
            let interactor = Interactor()
            let other = interactor.uiKit.window(hostedIn: 41)
            let selected = interactor.uiKit.window(hostedIn: 42)
            interactor.uiKit.key = other

            let change = try interactor.makeKey(window: 42)

            #expect(interactor.uiKit.madeKey.count == 1)
            #expect(interactor.uiKit.madeKey.first === selected)
            #expect(change.window.handle == interactor.handle(for: selected))
            #expect(change.window.windowId.value == 42)
            #expect(change.before.value?.handle == interactor.handle(for: other))
            #expect(change.before.value?.windowId.value == 41)
            // What the caller checks: the new key window identifies the window it selected.
            #expect(change.after.value?.handle == change.window.handle)
            #expect(change.after.value?.windowId.value == 42)
        }

        @Test("A window the target already has key is reported without another makeKey")
        func reportsAnUnchangedKeyWindow() throws {
            let interactor = Interactor()
            interactor.uiKit.key = interactor.uiKit.window(hostedIn: 7)

            let change = try interactor.makeKey(window: 7)

            #expect(interactor.uiKit.madeKey.isEmpty)
            #expect(change.before.value?.handle == change.window.handle)
            #expect(change.after.value?.handle == change.window.handle)
        }

        @Test("A target with no key window reports none before the change")
        func reportsNoPreviousKeyWindow() throws {
            let interactor = Interactor()
            let selected = interactor.uiKit.window(hostedIn: 9)
            interactor.uiKit.key = nil

            let change = try interactor.makeKey(window: 9)

            #expect(change.before.value == nil)
            #expect(change.after.value?.handle == interactor.handle(for: selected))
        }

        @Test("A change UIKit does not honour is reported, not claimed")
        func reportsAnUnhonouredChange() throws {
            let interactor = Interactor()
            let other = interactor.uiKit.window(hostedIn: 1)
            interactor.uiKit.window(hostedIn: 2)
            interactor.uiKit.key = other
            // UIKit keeps the key window it had, as a target that refuses the change leaves it.
            interactor.uiKit.keyAfterMakeKey = { _ in other }

            let change = try interactor.makeKey(window: 2)

            #expect(change.after.value?.handle == interactor.handle(for: other))
            #expect(change.after.value?.handle != change.window.handle)
        }

        @Test("A WindowServer window no UIWindow is hosted in changes nothing")
        func refusesAnUnhostedWindow() {
            let interactor = Interactor()
            interactor.uiKit.window(hostedIn: 10)

            #expect(throws: ReconActionError.self) { try interactor.makeKey(window: 11) }
            #expect(interactor.uiKit.madeKey.isEmpty)
        }

        @Test("A UIWindow hosted in no WindowServer window is never the match")
        func refusesAnUnattachedWindow() {
            let interactor = Interactor()
            interactor.uiKit.window(hostedIn: nil)

            #expect(throws: ReconActionError.self) { try interactor.makeKey(window: 12) }
            #expect(interactor.uiKit.madeKey.isEmpty)
        }

        @Test("Two UIWindows hosted in the selected window leave the choice undecided")
        func refusesAnAmbiguousWindow() {
            let interactor = Interactor()
            interactor.uiKit.window(hostedIn: 20)
            interactor.uiKit.window(hostedIn: 20)

            #expect(throws: ReconActionError.self) { try interactor.makeKey(window: 20) }
            #expect(interactor.uiKit.madeKey.isEmpty)
        }

        @Test("A window id that is not a WindowServer window is refused before UIKit is read")
        func refusesANonPositiveWindowId() {
            let interactor = Interactor()
            interactor.uiKit.windowsError = ReconActionError.unavailable("UIKit was read")

            let failure = #expect(throws: ReconActionError.self) {
                try interactor.makeKey(window: 0)
            }
            #expect(failure == .invalidArgument("A WindowServer window id is a positive integer"))
            #expect(interactor.uiKit.madeKey.isEmpty)
        }

        @Test("A cancelled invocation makes no change")
        func refusesACancelledInvocation() {
            let interactor = Interactor()
            interactor.uiKit.window(hostedIn: 30)

            let failure = #expect(throws: ReconActionError.self) {
                try interactor.makeKey(window: 30, cancelled: true)
            }
            #expect(failure == .cancelled)
            #expect(interactor.uiKit.madeKey.isEmpty)
        }

        @Test("A UIKit that cannot be read changes nothing")
        func refusesAnUnreadableUIKit() {
            let interactor = Interactor()
            interactor.uiKit.windowsError = TargetRefusedMessage(what: "a read of its scenes")

            #expect(throws: TargetRefusedMessage.self) { try interactor.makeKey(window: 40) }
            #expect(interactor.uiKit.madeKey.isEmpty)
        }

        @Test("A key window that cannot be read stops the operation before the change")
        func refusesAnUnreadableKeyWindow() {
            let interactor = Interactor()
            interactor.uiKit.window(hostedIn: 50)
            interactor.uiKit.keyWindowError = ReconActionError.unavailable("no key window")

            #expect(throws: ReconActionError.self) { try interactor.makeKey(window: 50) }
            #expect(interactor.uiKit.madeKey.isEmpty)
        }

        @Test("A refused makeKey is a failure, not a reported change")
        func reportsARefusedMakeKey() {
            let interactor = Interactor()
            interactor.uiKit.window(hostedIn: 60)
            interactor.uiKit.makeKeyError = TargetRefusedMessage(what: "makeKey()")

            #expect(throws: TargetRefusedMessage.self) { try interactor.makeKey(window: 60) }
        }

        @Test("One live window keeps one handle across the whole change")
        func namesOneWindowOnce() throws {
            let interactor = Interactor()
            interactor.uiKit.key = interactor.uiKit.window(hostedIn: 70)

            let change = try interactor.makeKey(window: 70)

            #expect(change.window.handle == change.before.value?.handle)
            #expect(change.window.handle == change.after.value?.handle)
        }
    }
}
