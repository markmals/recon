// swift-tools-version: 6.2
// Feasibility prototype for proposal.0006. Not a production SDK.
import CompilerPluginSupport
import PackageDescription

let package = Package(
    name: "ReconSDK",
    platforms: [.macOS("27.0")],
    products: [
        .library(name: "ReconExtension", targets: ["ReconExtension"]),
        .library(name: "ReconExtensionRuntime", targets: ["ReconExtensionRuntime"]),
        .library(name: "ReconNativeSupport", targets: ["ReconNativeSupport"]),
    ],
    dependencies: [
        .package(url: "https://github.com/swiftlang/swift-syntax.git", exact: "604.0.0"),
    ],
    targets: [
        .macro(
            name: "ReconExtensionMacros",
            dependencies: [
                .product(name: "SwiftSyntaxMacros", package: "swift-syntax"),
                .product(name: "SwiftSyntaxBuilder", package: "swift-syntax"),
                .product(name: "SwiftDiagnostics", package: "swift-syntax"),
                .product(name: "SwiftCompilerPlugin", package: "swift-syntax"),
            ]
        ),
        .target(name: "ReconExtension", dependencies: ["ReconExtensionMacros"]),
        .target(name: "ReconExtensionRuntime", dependencies: ["ReconExtension"]),
        .target(
            name: "ReconNativeSupport",
            linkerSettings: [.linkedFramework("AppKit")]
        ),
        .testTarget(
            name: "ReconExtensionTests",
            dependencies: ["ReconExtension", "ReconExtensionRuntime"]
        ),
    ]
)
