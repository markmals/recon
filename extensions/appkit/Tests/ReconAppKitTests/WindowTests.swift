import AppKit
import ReconNativeSupport
import Testing

@testable import ReconAppKit

/// Windows, the identity a WindowServer identifier names, and the session a handle belongs to.
extension AppKitInspection {
    @Suite
    struct Windows {
        @Test func everyApplicationWindowIsListedWithItsMapping() throws {
            let inspector = try Inspector()
            let window = onMain { Self.makeWindow(titled: "Fixture") }
            defer { onMain { window.close() } }

            let listed = try inspector.read("windows", [:], as: WindowsResponse.self)
            let record = try #require(listed.windows.first { $0.handle == inspector.handle(for: window) })
            #expect(record.kind == "window")
            #expect(record.basic?.title?.value == "Fixture")
            #expect(record.basic?.contentView?.handle != nil)
            // An off-screen window is not one the WindowServer reports as this process's.
            #expect(record.windowId?.number == nil)
            #expect(record.mapping == nil)
            #expect(listed.observation.source == "native-appkit")
            #expect(listed.observation.sessionId == "test-session")
            #expect(listed.observation.helperBuild == "test-build")
            #expect(listed.observation.kind == "window")
            #expect(listed.observation.identity.pid == ReconProcessIdentity.current.pid)
        }

        @Test func anUnmappableWindowIdentifierNamesTheStepThatFailed() throws {
            let inspector = try Inspector()
            let failure = try #require(try inspector.failure("tree", ["windowId": 999_999_999]))
            #expect(failure.code == "INSPECTION_UNAVAILABLE")
            #expect(failure.stage == "window-server-owner")
            #expect(failure.message.contains("No live owned native window matches"))
        }

        @Test func aWindowsOwnFrameIsTheGlobalBottomLeftRectangle() throws {
            let inspector = try Inspector()
            let window = onMain { Self.makeWindow(titled: "Framed") }
            defer { onMain { window.close() } }

            let node = try inspector.read(
                "node", ["root": inspector.handle(for: window), "fields": ["basic"]],
                as: NodeResponse.self
            ).node
            #expect(node.basic?.frame?.space == "global-screen")
            #expect(node.basic?.frame?.orientation == "bottom-left")
            #expect(node.basic?.normalizedFrame?.screenTopLeft?.orientation == "top-left")
            #expect(node.basic?.visible == false)
            #expect(node.basic?.windowNumber?.status == .absent)
            #expect(node.basic?.childWindows == [])
        }

        @Test func aViewInAWindowPublishesEveryNormalizedSpace() throws {
            let inspector = try Inspector()
            let window = onMain { Self.makeWindow(titled: "Normalized") }
            defer { onMain { window.close() } }
            let view = onMain { NSView(frame: NSRect(x: 5, y: 5, width: 20, height: 10)) }
            onMain { window.contentView?.addSubview(view) }

            let basic = try #require(
                try inspector.read(
                    "node", ["root": inspector.handle(for: view), "fields": ["basic"]],
                    as: NodeResponse.self
                ).node.basic)
            #expect(basic.window?.handle == inspector.handle(for: window))
            #expect(basic.normalizedFrame?.window?.space == "window")
            #expect(basic.normalizedFrame?.screen?.space == "global-screen")
            #expect(basic.normalizedFrame?.transform?.method == "primary-screen-height-flip")
        }

        @Test func handlesBelongToTheSessionThatIssuedThem() throws {
            let first = try Inspector("session-one")
            let view = onMain { NSView() }
            let handle = first.handle(for: view)
            #expect(try first.failure("node", ["root": handle]) == nil)

            let second = try Inspector("session-two")
            let failure = try #require(try second.failure("node", ["root": handle]))
            #expect(failure.code == "STALE_NODE")
        }

        @MainActor
        private static func makeWindow(titled title: String) -> NSWindow {
            let window = NSWindow(
                contentRect: NSRect(x: 0, y: 0, width: 160, height: 120), styleMask: [.titled],
                backing: .buffered, defer: true)
            window.title = title
            return window
        }
    }
}
