import AppKit
import Testing

@testable import ReconAppKit

/// What the largest read a client can ask for costs. This is a measurement for the record, not
/// a gate: it prints what one tree at the visit cap, with every field group, takes to answer.
extension AppKitInspection {
    @Suite
    struct ReadCost {
        /// Every field group, over as many controls as the 8 MiB response bound leaves room
        /// for. The 25,000-visit cap is out of reach: records that large hit the bound first.
        @Test func aTreeWithEveryFieldGroup() throws {
            let answered = try measure(
                "every field group", nodes: 1_500, controls: true,
                fields: ["basic", "appearance", "font", "layout", "layer", "cell"])
            #expect(answered.nodes.count == 1_501)
        }

        /// The widest traversal the bound allows, which is what the visit budget is there for.
        @Test func aWideTreeOfPlainViews() throws {
            let answered = try measure(
                "widest traversal", nodes: 6_000, controls: false, fields: ["basic"])
            #expect(answered.coverage.visited == 12_001)
        }

        private func measure(_ label: String, nodes: Int, controls: Bool, fields: [String]) throws
            -> NodesResponse
        {
            let inspector = try Inspector()
            let parent = onMain { NSView() }
            let children = onMain {
                Carrier(
                    (0..<nodes).map { _ in controls ? NSTextField(labelWithString: "node") : NSView() })
            }
            onMain { children.value.forEach(parent.addSubview) }

            var tree: NodesResponse?
            let elapsed = try ContinuousClock().measure {
                tree = try inspector.read(
                    "tree",
                    [
                        "root": inspector.handle(for: parent), "depth": 1, "limit": 25_000,
                        "fields": fields,
                    ], as: NodesResponse.self)
            }
            let answered = try #require(tree)
            print(
                "read cost (\(label)): \(answered.nodes.count) nodes, "
                    + "\(answered.coverage.visited) visits, \(elapsed)")
            return answered
        }
    }
}
