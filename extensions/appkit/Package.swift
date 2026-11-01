// swift-tools-version: 6.2
// Feasibility prototype for proposal.0006: a Swift AppKit action assembled into the target helper.
import PackageDescription

let package = Package(
    name: "ReconAppKit",
    platforms: [.macOS("27.0")],
    products: [
        .library(name: "ReconHelper", type: .dynamic, targets: ["ReconHelper"]),
    ],
    dependencies: [
        .package(path: "../../sdks/swift"),
    ],
    targets: [
        .target(
            name: "ReconAppKit",
            dependencies: [
                .product(name: "ReconExtension", package: "swift"),
                .product(name: "ReconNativeSupport", package: "swift"),
            ]
        ),
        .target(
            name: "ReconHelperServer",
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("Security"),
                .linkedLibrary("bsm"),
                .linkedLibrary("proc"),
            ]
        ),
        .target(
            name: "ReconHelper",
            dependencies: [
                "ReconAppKit",
                "ReconHelperServer",
                .product(name: "ReconExtensionRuntime", package: "swift"),
            ]
        ),
    ]
)
