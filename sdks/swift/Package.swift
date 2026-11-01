// swift-tools-version: 6.2
// Recon's Swift extension-authoring SDK (proposal.0006). Built only by Apple's pinned Swift
// compiler; `mise run swift:toolchain` verifies it.
import CompilerPluginSupport
import PackageDescription

// Importing AppKit into a Swift Testing bundle pulls in swift-testing's AppKit cross-import
// overlay, which links SwiftUICore — a framework the SDK lets only SwiftUI link. Recon's tests
// use none of that overlay's conveniences.
let appKitTestSettings: [SwiftSetting] = [.unsafeFlags(["-disable-cross-import-overlays"])]

let package = Package(
    name: "ReconSDK",
    platforms: [.macOS("27.0")],
    products: [
        // What an extension author imports.
        .library(name: "ReconExtension", targets: ["ReconExtension"]),
        .library(name: "ReconExtensionTesting", targets: ["ReconExtensionTesting"]),
        .library(name: "ReconNativeSupport", targets: ["ReconNativeSupport"]),
        // What the target-resident helper assembles and serves actions with.
        .library(name: "ReconExtensionRuntime", targets: ["ReconExtensionRuntime"]),
    ],
    dependencies: [
        .package(url: "https://github.com/swiftlang/swift-syntax.git", exact: "604.0.0")
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
        .target(name: "ReconExtensionTesting", dependencies: ["ReconExtensionRuntime"]),
        // The exception boundary: typed message-send trampolines and nothing else. Which
        // messages may be sent, and what their results mean, is Swift in ReconNativeSupport.
        .target(name: "ReconNativeAccessors"),
        .target(
            name: "ReconNativeSupport",
            dependencies: ["ReconExtension", "ReconNativeAccessors"],
            linkerSettings: [
                .linkedFramework("AppKit"),
                .linkedFramework("QuartzCore"),
                .linkedFramework("CoreGraphics"),
            ]
        ),
        .testTarget(
            name: "ReconExtensionTests",
            dependencies: ["ReconExtension", "ReconExtensionRuntime", "ReconExtensionTesting"]
        ),
        .testTarget(
            name: "ReconNativeSupportTests",
            dependencies: ["ReconNativeSupport", "ReconExtensionTesting"]
        ),
        // Compiles the guide's examples exactly as printed and checks they behave as printed.
        .testTarget(
            name: "ReconGuideTests",
            dependencies: ["ReconExtension", "ReconExtensionTesting", "ReconNativeSupport"],
            swiftSettings: appKitTestSettings
        ),
        // Compiles rejected snippets with the real compiler and plugin, and checks the
        // diagnostics an author reads — never the generated source.
        .testTarget(name: "ReconDiagnosticsTests", dependencies: ["ReconExtension"]),
    ]
)
