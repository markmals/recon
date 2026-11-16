// swift-tools-version: 6.2
// The target-resident helper: Recon's loader injects this dylib, and it serves the assembled
// extensions' actions over the authenticated socket (proposal.0006).
//
// `mise run helper:build` builds it universal for arm64 and arm64e with the pinned Apple Swift
// compiler, and writes the build identity the CLI must match.
import PackageDescription

let package = Package(
    name: "ReconHelper",
    platforms: [.macOS("27.0")],
    products: [
        .library(name: "ReconHelper", type: .dynamic, targets: ["ReconHelper"])
    ],
    dependencies: [
        .package(path: "../sdks/swift"),
        .package(path: "../extensions/appkit"),
        .package(path: "../extensions/catalyst"),
    ],
    targets: [
        .target(
            name: "ReconHelper",
            dependencies: [
                .product(name: "ReconAppKit", package: "appkit"),
                .product(name: "ReconCatalyst", package: "catalyst"),
                .product(name: "ReconExtensionRuntime", package: "swift"),
                .product(name: "ReconNativeSupport", package: "swift"),
            ]
        ),
        .testTarget(name: "ReconHelperTests", dependencies: ["ReconHelper"]),
    ]
)
