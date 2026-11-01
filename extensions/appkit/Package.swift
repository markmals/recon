// swift-tools-version: 6.2
// Recon's AppKit inspector: the first consumer of the Swift extension SDK (proposal.0006).
// The helper package assembles this library; it is not a product on its own.
import PackageDescription

// Importing AppKit into a Swift Testing bundle pulls in swift-testing's AppKit cross-import
// overlay, which links SwiftUICore — a framework the SDK lets only SwiftUI link. These tests
// use none of that overlay's conveniences.
let appKitTestSettings: [SwiftSetting] = [.unsafeFlags(["-disable-cross-import-overlays"])]

let package = Package(
    name: "ReconAppKit",
    platforms: [.macOS("27.0")],
    products: [
        .library(name: "ReconAppKit", targets: ["ReconAppKit"])
    ],
    dependencies: [
        .package(path: "../../sdks/swift")
    ],
    targets: [
        .target(
            name: "ReconAppKit",
            dependencies: [
                .product(name: "ReconExtension", package: "swift"),
                .product(name: "ReconNativeSupport", package: "swift"),
            ]
        ),
        .testTarget(
            name: "ReconAppKitTests",
            dependencies: [
                "ReconAppKit",
                .product(name: "ReconExtensionTesting", package: "swift"),
            ],
            swiftSettings: appKitTestSettings
        ),
    ]
)
