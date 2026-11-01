import AppKit
import ReconNativeSupport
import Testing

@testable import ReconAppKit

/// The facts each field group publishes, and the distinctions proposal.0005 insists on between
/// a value, an absence, an unsupported read, and a getter that failed.
extension AppKitInspection {
    @Suite
    struct Facets {
        @Test func accessibilityPropertiesAreTheObjectsOwnAndNotItsIdentifier() throws {
            let inspector = try Inspector()
            let view = onMain { NSView() }
            onMain {
                view.identifier = NSUserInterfaceItemIdentifier("native-identifier")
                view.setAccessibilityIdentifier("ax-identifier")
                view.setAccessibilityRole(.button)
            }
            let basic = try #require(
                try inspector.read(
                    "node", ["root": inspector.handle(for: view), "fields": ["basic"]],
                    as: NodeResponse.self
                ).node.basic)
            #expect(basic.identifier?.value == "native-identifier")
            #expect(basic.accessibilityIdentifier?.value == "ax-identifier")
            #expect(basic.accessibilityIdentifier?.read == "target-executed")
            #expect(basic.accessibilityRole?.value == NSAccessibility.Role.button.rawValue)
            // A view in no window has no normalization to report, which is an absence.
            #expect(basic.normalizedFrame?.status == .absent)
        }

        @Test func anEmptyAccessibilityIdentifierIsAnAbsenceRatherThanAValue() throws {
            let inspector = try Inspector()
            let view = onMain { NSView() }
            let basic = try #require(
                try inspector.read(
                    "node", ["root": inspector.handle(for: view), "fields": ["basic"]],
                    as: NodeResponse.self
                ).node.basic)
            #expect(basic.accessibilityIdentifier?.status == .absent)
            #expect(basic.accessibilityIdentifier?.value == nil)
        }

        @Test func aRaisingGetterFailsOneFacetAndNotTheRead() throws {
            let inspector = try Inspector()
            let parent = onMain { NSView() }
            let raising = onMain { RaisingView() }
            let plain = onMain { NSView() }
            onMain {
                parent.addSubview(raising)
                parent.addSubview(plain)
            }
            let tree = try inspector.read(
                "tree", ["root": inspector.handle(for: parent), "depth": 1, "fields": ["basic"]],
                as: NodesResponse.self)
            #expect(tree.nodes.count == 3)
            let failed = try #require(tree.nodes.first { $0.handle == inspector.handle(for: raising) })
            #expect(failed.basic?.status == .failed)
            #expect(failed.basic?.reason == "target-getter-exception")
            let intact = try #require(tree.nodes.first { $0.handle == inspector.handle(for: plain) })
            #expect(intact.basic?.status == .value)
        }

        @Test func aTypeCheckIsNeverSentToTheTargetsOwnOverride() throws {
            let inspector = try Inspector()
            let parent = onMain { NSView() }
            let raising = onMain { RaisingKindView() }
            onMain { parent.addSubview(raising) }
            let root = inspector.handle(for: parent)
            let child = inspector.handle(for: raising)
            RaisingKindView.armed = true
            defer { RaisingKindView.armed = false }
            let tree = try inspector.read(
                "tree", ["root": root, "depth": 1, "fields": ["basic"]], as: NodesResponse.self)
            let node = try #require(tree.nodes.first { $0.handle == child })
            #expect(node.kind == "view")
        }

        @Test func onlyTheRequestedFieldsAreRead() throws {
            let inspector = try Inspector()
            let parent = onMain { NSView() }
            let child = onMain { NSView() }
            onMain { parent.addSubview(child) }
            let node = try inspector.read(
                "node", ["root": inspector.handle(for: child), "fields": ["appearance"]],
                as: NodeResponse.self
            ).node
            #expect(node.appearance != nil)
            #expect(node.basic == nil)
            #expect(node.relationships?.superview?.handle == inspector.handle(for: parent))
            #expect(node.relationships?.window?.handle == nil)
        }

        @Test func aLayoutGuideIsItsOwnKindOfNode() throws {
            let inspector = try Inspector()
            let parent = onMain { NSView() }
            let guide = onMain { NSLayoutGuide() }
            onMain {
                guide.identifier = NSUserInterfaceItemIdentifier("fixture-guide")
                parent.addLayoutGuide(guide)
            }
            let root = inspector.handle(for: parent)
            let layout = try #require(
                try inspector.read("node", ["root": root, "fields": ["layout"]], as: NodeResponse.self)
                    .node.layout)
            #expect(layout.layoutGuides?.count == 1)
            let handle = try #require(layout.layoutGuides?[0].handle.handle)

            let node = try inspector.read(
                "node", ["root": handle, "fields": ["basic", "layout"]], as: NodeResponse.self
            ).node
            #expect(node.kind == "layoutGuide")
            #expect(node.relationships?.owningView?.handle == root)
            #expect(node.layout?.hugging?.status == .unsupported)
            #expect(node.layout?.intrinsicWidth?.status == .unsupported)
        }

        @Test func aViewPublishesItsIntrinsicSizeAndPriorities() throws {
            let inspector = try Inspector()
            let field = onMain { NSTextField(labelWithString: "hello") }
            let layout = try #require(
                try inspector.read(
                    "node", ["root": inspector.handle(for: field), "fields": ["layout"]],
                    as: NodeResponse.self
                ).node.layout)
            #expect(layout.intrinsicWidth?.status == .value)
            #expect(layout.hugging?.horizontal?.number != nil)
            #expect(layout.compressionResistance?.vertical?.number != nil)
        }

        @Test func aControlPublishesItsCellAndItsFont() throws {
            let inspector = try Inspector()
            let field = onMain { NSTextField(labelWithString: "hello") }
            let node = try inspector.read(
                "node", ["root": inspector.handle(for: field), "fields": ["cell", "font", "layer"]],
                as: NodeResponse.self
            ).node
            #expect(node.cell?.status == .value)
            #expect(node.cell?.font?.status == .value)
            #expect(node.font?.carrier?.status == .value)
            #expect(node.font?.source == "view")
            #expect(node.layer?.status == .absent || node.layer?.status == .value)
        }

        /// A colour read raises inside AppKit for every colour, so the facet that asks for one
        /// reports a failed target getter. That is the behaviour this port reproduces.
        @Test func aFacetThatReadsAColourReportsTheGetterItFailedOn() throws {
            let inspector = try Inspector()
            let field = onMain { NSTextField(labelWithString: "hello") }
            let appearance = try #require(
                try inspector.read(
                    "node", ["root": inspector.handle(for: field), "fields": ["appearance"]],
                    as: NodeResponse.self
                ).node.appearance)
            #expect(appearance.status == .failed)
            #expect(appearance.reason == "target-getter-exception")
        }

        @Test func aPlainViewsAppearanceIsReadable() throws {
            let inspector = try Inspector()
            let view = onMain { NSView() }
            let appearance = try #require(
                try inspector.read(
                    "node", ["root": inspector.handle(for: view), "fields": ["appearance"]],
                    as: NodeResponse.self
                ).node.appearance)
            #expect(appearance.status == .value)
            #expect(appearance.effectiveAppearance?.text != nil)
        }

        @Test func anAttributedValueIsBoundedByTheRunsItExamines() throws {
            let inspector = try Inspector()
            let cell = onMain { AlternatingFontCell() }
            let fonts = try inspector.read(
                "node", ["root": inspector.handle(for: cell), "fields": ["font"]],
                as: NodeResponse.self)
            #expect(fonts.coverage.visited <= Work.runLimit)
            #expect(fonts.coverage.truncated.contains("attributed-runs:\(Work.runLimit)"))
            #expect(fonts.node.font?.source == "cell")
        }

        @Test func theScreenFlipIsTheScreensHeightAndNothingElse() {
            #expect(
                ReconNative.screenTopLeft(NSRect(x: 10, y: 100, width: 50, height: 20),
                    primaryScreenHeight: 900) == NSRect(x: 10, y: 780, width: 50, height: 20))
            #expect(
                ReconNative.screenTopLeft(NSRect(x: -1520, y: 1056, width: 80, height: 24),
                    primaryScreenHeight: 900) == NSRect(x: -1520, y: -180, width: 80, height: 24))
            let retina = NSRect(x: 12.5, y: 33.25, width: 100.5, height: 50.75)
            #expect(
                ReconNative.screenTopLeft(
                    ReconNative.screenTopLeft(retina, primaryScreenHeight: 1050.5),
                    primaryScreenHeight: 1050.5) == retina)
        }
    }
}
