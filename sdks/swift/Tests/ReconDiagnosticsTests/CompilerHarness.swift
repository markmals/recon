import Foundation

/// Compiles a snippet with the real compiler and the real macro plugin, and answers what the
/// compiler said. The point is the message an author reads, so nothing here inspects generated
/// source.
enum Compiler {
    static func diagnostics(for snippet: String) throws -> String {
        let directory = FileManager.default.temporaryDirectory.appending(
            path: "recon-diagnostics-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let source = directory.appending(path: "Snippet.swift")
        try "import ReconExtension\n\n\(snippet)".write(to: source, atomically: true, encoding: .utf8)

        let products = try self.products
        let process = Process()
        process.executableURL = URL(filePath: "/usr/bin/xcrun")
        process.arguments = [
            "swiftc", "-typecheck", "-swift-version", "6", "-I", products.path(),
            "-load-plugin-executable", "\(products.path())/ReconExtensionMacros#ReconExtensionMacros",
            source.path(),
        ]
        let output = Pipe()
        process.standardOutput = output
        process.standardError = output
        try process.run()
        let text = String(
            decoding: output.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
        process.waitUntilExit()
        return process.terminationStatus == 0 ? "" : text
    }

    /// The directory holding this test run's own build products, found by walking up from this
    /// test bundle until the macro plugin appears beside it.
    private static var products: URL {
        get throws {
            var directory = Bundle(for: BundleToken.self).bundleURL
            while directory.path() != "/" {
                let plugin = directory.appending(path: "ReconExtensionMacros")
                if FileManager.default.isExecutableFile(atPath: plugin.path()) { return directory }
                directory = directory.deletingLastPathComponent()
            }
            throw CompilerUnavailable()
        }
    }
}

private final class BundleToken {}

struct CompilerUnavailable: Error {}
