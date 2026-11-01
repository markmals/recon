import AppKit
import Testing

@testable import ReconAppKit

/// The traversal behaviour proposal.0005 publishes, read through the real runtime so that every
/// result is also checked against the schema its action publishes.
extension AppKitInspection {
    @Suite
    struct Traversal {
        @Test func aDepthOfZeroReportsOnlyTheRootAndSaysItNarrowed() throws {
            let inspector = try Inspector()
            let parent = onMain { NSView() }
            let child = onMain { NSView() }
            onMain { parent.addSubview(child) }
            let root = inspector.handle(for: parent)

            let tree = try inspector.read(
                "tree", ["root": root, "depth": 0, "fields": ["basic"]], as: NodesResponse.self)
            #expect(tree.nodes.count == 1)
            #expect(tree.coverage.narrowed.contains("depth"))
            #expect(!tree.coverage.complete || tree.coverage.truncated.isEmpty)
        }

        @Test func findMatchesAnExactIdentifier() throws {
            let inspector = try Inspector()
            let parent = onMain { NSView() }
            let child = onMain { NSView() }
            onMain {
                child.identifier = NSUserInterfaceItemIdentifier("child")
                parent.addSubview(child)
            }
            let root = inspector.handle(for: parent)

            let found = try inspector.read(
                "find", ["root": root, "identifier": "child", "depth": 2, "fields": ["basic"]],
                as: NodesResponse.self)
            #expect(found.nodes.count == 1)
            #expect(found.nodes[0].handle == inspector.handle(for: child))

            let missing = try inspector.read(
                "find", ["root": root, "identifier": "nomatch", "depth": 2, "fields": ["basic"]],
                as: NodesResponse.self)
            #expect(missing.nodes.isEmpty)
        }

        @Test func findWithoutAFilterIsRejected() throws {
            let inspector = try Inspector()
            let view = onMain { NSView() }
            let root = inspector.handle(for: view)
            let failure = try #require(try inspector.failure("find", ["root": root]))
            #expect(failure.code == "INVALID_ARGUMENT")
            #expect(failure.stage == "filters")
        }

        @Test func aRepeatedObjectIsReportedOnceAndThenReferenced() throws {
            let inspector = try Inspector()
            let parent = onMain { RepeatingView() }
            let member = onMain { NSView() }
            onMain {
                member.identifier = NSUserInterfaceItemIdentifier("member")
                parent.addSubview(member)
            }
            let root = inspector.handle(for: parent)

            let limited = try inspector.read(
                "tree", ["root": root, "depth": 1, "limit": 2, "fields": ["appearance"]],
                as: NodesResponse.self)
            #expect(limited.nodes.count == 2)
            #expect(limited.coverage.truncated.contains("unsearched-scope"))

            let full = try inspector.read(
                "tree", ["root": root, "depth": 1, "limit": 3, "fields": ["appearance"]],
                as: NodesResponse.self)
            #expect(full.nodes.count == 3)
            #expect(full.coverage.returned == 3)
            #expect(full.nodes[2].reference == true)
            #expect(full.nodes[2].handle == full.nodes[1].handle)
        }

        @Test func aLayerTraversalNeedsALayerScope() throws {
            let inspector = try Inspector()
            let failure = try #require(try inspector.failure("tree", ["kind": "layer"]))
            #expect(failure.code == "INVALID_ARGUMENT")
            #expect(failure.stage == "scope")

            let view = onMain { NSView() }
            let scoped = try inspector.read(
                "tree",
                ["root": inspector.handle(for: view), "kind": "layer", "fields": ["basic"]],
                as: NodesResponse.self)
            #expect(scoped.nodes.allSatisfy { $0.kind == "layer" })
        }

        @Test func aStaleHandleIsNotAnEmptyObservation() throws {
            let inspector = try Inspector()
            let view = onMain { NSView() }
            let handle = inspector.handle(for: view)
            inspector.session.handles.revoke()
            let failure = try #require(try inspector.failure("node", ["root": handle]))
            #expect(failure.code == "STALE_NODE")
            #expect(failure.stage == "root")
        }

        @Test func aMalformedHandleIsRejectedBeforeAnythingIsRead() throws {
            let inspector = try Inspector()
            let failure = try #require(try inspector.failure("node", ["root": "n:invalid"]))
            #expect(failure.code == "INVALID_ARGUMENT")
            #expect(failure.stage == "root")
            #expect(try inspector.failure("node", [:])?.stage == "root")
        }

        @Test func boundsAreRefusedBeforeTheTargetIsTouched() throws {
            let inspector = try Inspector()
            let view = onMain { NSView() }
            let root = inspector.handle(for: view)
            for payload in [
                ["root": root, "depth": 65], ["root": root, "limit": 0],
                ["root": root, "limit": 25001], ["root": root, "fields": []],
                ["root": root, "fields": ["unknown"]], ["root": root, "fields": ["basic", "basic"]],
            ] as [[String: Any]] {
                let failure = try #require(try inspector.failure("node", payload))
                #expect(failure.code == "INVALID_ARGUMENT")
                #expect(failure.stage == "bounds")
            }
            #expect(try inspector.failure("node", ["root": root, "kind": "sideways"])?.stage == "kind")
            #expect(
                try inspector.failure("node", ["root": root, "windowId": 0])?.stage == "window")
        }
    }
}
