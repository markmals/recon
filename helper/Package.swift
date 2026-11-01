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
    ],
    targets: [
        // Authentication, the access lifecycle, framing, and the loader's entry point stay in
        // Objective-C: they are the boundary the loader and the host already trust.
        .target(
            name: "ReconHelperServer",
            dependencies: [
                .product(name: "ReconNativeAccessors", package: "swift")
            ],
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("Security"),
                .linkedLibrary("bsm"),
            ]
        ),
        .target(
            name: "ReconHelper",
            dependencies: [
                .product(name: "ReconAppKit", package: "appkit"),
                .product(name: "ReconExtensionRuntime", package: "swift"),
                "ReconHelperServer",
            ]
        ),
    ]
)
