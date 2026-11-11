// swift-tools-version: 6.2
// Recon's Catalyst interaction extension (proposal.0007): the helper's one closed interaction
// operation, make-window-key. The helper package assembles this library; it is not a product on
// its own.
import PackageDescription

let package = Package(
    name: "ReconCatalyst",
    platforms: [.macOS("27.0")],
    products: [
        .library(name: "ReconCatalyst", targets: ["ReconCatalyst"])
    ],
    dependencies: [
        .package(path: "../../sdks/swift")
    ],
    targets: [
        // The Objective-C boundary this extension reaches UIKit through. The helper stays a
        // macOS library and links no UIKit: every class and selector here is resolved in the
        // target at run time, and an AppKit target resolves none of them.
        .target(name: "ReconCatalystAccessors"),
        .target(
            name: "ReconCatalyst",
            dependencies: [
                "ReconCatalystAccessors",
                .product(name: "ReconExtension", package: "swift"),
                .product(name: "ReconNativeSupport", package: "swift"),
            ]
        ),
        .testTarget(
            name: "ReconCatalystTests",
            dependencies: [
                "ReconCatalyst",
                .product(name: "ReconExtensionTesting", package: "swift"),
            ]
        ),
    ]
)
