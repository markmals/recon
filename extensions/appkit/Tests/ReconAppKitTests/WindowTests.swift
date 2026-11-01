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

        /// Every rectangle is published in the space it was read in, at the coordinates that
        /// space gives it: a view's own bounds, its place in the window, and the same place on
        /// the global screen are three different answers.
        @Test func rectanglesCarryTheirOwnSpacesCoordinates() throws {
            let inspector = try Inspector()
            let window = onMain { Self.makeWindow(titled: "Placed") }
            defer { onMain { window.close() } }
            let view = onMain { NSView(frame: NSRect(x: 5, y: 7, width: 20, height: 10)) }
            onMain {
                window.setFrameOrigin(NSPoint(x: 300, y: 200))
                window.contentView?.addSubview(view)
            }
            let origin = onMainValue { [window.frame.origin.x, window.frame.origin.y] }

            let basic = try #require(
                try inspector.read(
                    "node", ["root": inspector.handle(for: view), "fields": ["basic"]],
                    as: NodeResponse.self
                ).node.basic)
            #expect(basic.frame?.orientation == "bottom-left")
            #expect(basic.frame?.x == 5)
            #expect(basic.frame?.y == 7)
            #expect(basic.bounds?.space == "self")
            #expect(basic.bounds?.orientation == "bottom-left")
            #expect(basic.bounds?.x == 0)
            #expect(basic.bounds?.y == 0)
            #expect(basic.bounds?.width == 20)
            let inWindow = try #require(basic.normalizedFrame?.window)
            let onScreen = try #require(basic.normalizedFrame?.screen)
            #expect(inWindow.x == 5)
            #expect(inWindow.y == 7)
            // The same rectangle on the global screen is the window's own origin away from it,
            // which is what tells the two spaces apart.
            #expect(try #require(onScreen.x) - #require(inWindow.x) == origin[0])
            #expect(try #require(onScreen.y) - #require(inWindow.y) == origin[1])
        }

        /// A flipped view measures from its own top left, and says so.
        @Test func aFlippedViewReportsTopLeftBounds() throws {
            let inspector = try Inspector()
            let parent = onMain { FlippedView() }
            let child = onMain { NSView(frame: NSRect(x: 2, y: 3, width: 8, height: 4)) }
            onMain { parent.addSubview(child) }

            let parentBasic = try #require(
                try inspector.read(
                    "node", ["root": inspector.handle(for: parent), "fields": ["basic"]],
                    as: NodeResponse.self
                ).node.basic)
            #expect(parentBasic.flipped == true)
            #expect(parentBasic.bounds?.orientation == "top-left")

            let childBasic = try #require(
                try inspector.read(
                    "node", ["root": inspector.handle(for: child), "fields": ["basic"]],
                    as: NodeResponse.self
                ).node.basic)
            // The frame is measured in the superview's space, so a flipped parent flips it.
            #expect(childBasic.frame?.space == "superview")
            #expect(childBasic.frame?.orientation == "top-left")
            #expect(childBasic.bounds?.orientation == "bottom-left")
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
