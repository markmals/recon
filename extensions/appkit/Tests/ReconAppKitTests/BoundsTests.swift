import AppKit
import QuartzCore
import ReconExtension
import ReconNativeSupport
import Testing

@testable import ReconAppKit

/// What stops a read: the visit budget, the handle registry, and the published limit. Every
/// bound a traversal hits has to show up in the coverage it answers with.
extension AppKitInspection {
    @Suite
    struct Bounds {
        /// A layer may report any superlayer it likes, including one that leads back to it.
        /// Resolving a layer's window spends the read's budget so a cycle cannot spin the
        /// target's main thread.
        @Test func aCyclicSuperlayerChainStopsAtTheVisitBudget() throws {
            let inspector = try Inspector()
            let pair = onMain { Carrier((first: CyclicLayer(), second: CyclicLayer())) }
            onMain {
                pair.value.first.cycle = pair.value.second
                pair.value.second.cycle = pair.value.first
            }
            defer {
                onMain {
                    pair.value.first.cycle = nil
                    pair.value.second.cycle = nil
                }
            }
            let work = onMain { Carrier(try! inspector.work()) }

            let owned = onMainValue {
                ((try? work.value.windowOwning(pair.value.first)) ?? nil) != nil
            }
            #expect(!owned)
            #expect(work.value.visited == 25_000)
            #expect(work.value.truncated.contains("work:25000"))
        }

        /// A deep but finite chain costs one visit a layer and answers that no view backs it.
        @Test func aLongSuperlayerChainIsWalkedOnce() throws {
            let inspector = try Inspector()
            let layers = onMain { Carrier((0..<100).map { _ in CALayer() }) }
            onMain {
                for index in 1..<layers.value.count {
                    layers.value[index].addSublayer(layers.value[index - 1])
                }
            }
            let work = onMain { Carrier(try! inspector.work()) }

            let owned = onMainValue {
                ((try? work.value.windowOwning(layers.value[0])) ?? nil) != nil
            }
            #expect(!owned)
            #expect(work.value.visited == 100)
            #expect(work.value.truncated.isEmpty)
        }

        /// A read that runs out of handles cannot name what it found, so it ends rather than
        /// publishing records that point at nothing.
        @Test func anExhaustedRegistryEndsTheRead() throws {
            let inspector = try Inspector()
            var filler: [NSObject] = []
            filler.reserveCapacity(ReconHandleRegistry.capacity)
            for _ in 0..<ReconHandleRegistry.capacity {
                let object = NSObject()
                filler.append(object)
                _ = inspector.session.handles.handle(for: object)
            }
            let work = try inspector.work()
            let overflow = NSObject()

            #expect(work.reference(overflow) == nil)
            #expect(work.truncated.contains("registry:100000"))
            #expect(work.narrowed.isEmpty)
            #expect(throws: ReconActionError.self) { try work.coverage() }
        }

        /// A tree queues only what it can still return, so the work the target does is bounded
        /// by the limit rather than by the size of the subtree.
        @Test func aTreeQueuesNoMoreThanItCanReturn() throws {
            let inspector = try Inspector()
            let parent = onMain { NSView() }
            let children = onMain { Carrier((0..<10).map { _ in NSView() }) }
            onMain { children.value.forEach(parent.addSubview) }

            let tree = try inspector.read(
                "tree",
                ["root": inspector.handle(for: parent), "depth": 1, "limit": 3, "fields": ["basic"]],
                as: NodesResponse.self)
            #expect(tree.nodes.count == 3)
            #expect(tree.coverage.returned == 3)
            // Three records dequeued and two children queued: nothing is queued that the limit
            // would not let the read return.
            #expect(tree.coverage.visited == 5)
            #expect(tree.coverage.truncated.contains("returned:limit"))
            #expect(tree.coverage.complete == false)
        }

        /// A search that stops at its limit says the rest of the scope went unsearched, which
        /// is what keeps a short answer from reading as a complete one.
        @Test func aSearchThatStopsEarlySaysTheScopeWentUnsearched() throws {
            let inspector = try Inspector()
            let parent = onMain { NSView() }
            let children = onMain { Carrier((0..<5).map { _ in NSView() }) }
            onMain { children.value.forEach(parent.addSubview) }

            let found = try inspector.read(
                "find",
                [
                    "root": inspector.handle(for: parent), "class": "NSView", "depth": 1,
                    "limit": 2, "fields": ["basic"],
                ], as: NodesResponse.self)
            #expect(found.nodes.count == 2)
            #expect(found.coverage.truncated.contains("returned:limit"))
            #expect(found.coverage.truncated.contains("unsearched-scope"))
            #expect(found.coverage.complete == false)
        }
    }
}

/// Carries a collection of AppKit objects back from the main thread.
final class Carrier<Value>: @unchecked Sendable {
    let value: Value
    init(_ value: Value) { self.value = value }
}
