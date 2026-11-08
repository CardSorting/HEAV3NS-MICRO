// [LAYER: PLUMBING]
import Foundation

/// High-performance launcher and runtime resolver for HEAV3NS-MICRO and legacy Node fallbacks.
/// Automatically detects and prioritizes the hyper-compressed Rust native agent binary.
final class EngineManager {
    static let shared = EngineManager()

    var bundledCLIURL: URL {
        let bundle = Bundle.main.resourceURL ?? Bundle.main.bundleURL
        let nativeMicro = bundle.appendingPathComponent("runtime/heav3ns-micro")
        if FileManager.default.isExecutableFile(atPath: nativeMicro.path) {
            return nativeMicro
        }
        return bundle.appendingPathComponent("runtime/dist/index.js")
    }

    var bundledNodeURL: URL {
        (Bundle.main.resourceURL ?? Bundle.main.bundleURL).appendingPathComponent("runtime/node")
    }

    /// Resolves the executable to launch. Prioritizes HEAV3NS_MICRO, then HEAV3NS_CLI,
    /// then workspace target/release/heav3ns-micro, then bundled binary.
    func resolveExecutable(override: URL? = nil, environment: [String: String] = ProcessInfo.processInfo.environment) -> (url: URL, isDirectBinary: Bool, arguments: [String]) {
        if let override {
            let isBinary = !override.path.hasSuffix(".js")
            return (override, isBinary, isBinary ? ["--stdio"] : ["--desktop-worker"])
        }
        if let microPath = environment["HEAV3NS_MICRO"], !microPath.isEmpty {
            let url = URL(fileURLWithPath: microPath)
            if FileManager.default.isExecutableFile(atPath: url.path) {
                return (url, true, ["--stdio"])
            }
        }
        // Check relative target release build in current directory or ancestor
        let cwd = FileManager.default.currentDirectoryPath
        let candidates = [
            cwd + "/target/release/heav3ns-micro",
            cwd + "/target/debug/heav3ns-micro",
            (Bundle.main.bundleURL.deletingLastPathComponent().path) + "/target/release/heav3ns-micro"
        ]
        for candidate in candidates {
            if FileManager.default.isExecutableFile(atPath: candidate) {
                return (URL(fileURLWithPath: candidate), true, ["--stdio"])
            }
        }
        if let cliPath = environment["HEAV3NS_CLI"], !cliPath.isEmpty {
            let url = URL(fileURLWithPath: cliPath)
            if !url.path.hasSuffix(".js") && FileManager.default.isExecutableFile(atPath: url.path) {
                return (url, true, ["--stdio"])
            }
            let node = resolveNode(environment: environment)
            return (node, false, [url.path, "--desktop-worker"])
        }
        let bundled = bundledCLIURL
        if !bundled.path.hasSuffix(".js") && FileManager.default.isExecutableFile(atPath: bundled.path) {
            return (bundled, true, ["--stdio"])
        }
        let node = resolveNode(environment: environment)
        return (node, false, [bundled.path, "--desktop-worker"])
    }

    func resolveCLI(override: URL? = nil, environment: [String: String] = ProcessInfo.processInfo.environment) -> URL {
        if let override { return override }
        if let path = environment["HEAV3NS_CLI"], !path.isEmpty { return URL(fileURLWithPath: path) }
        return bundledCLIURL
    }

    func resolveNode(override: URL? = nil, environment: [String: String] = ProcessInfo.processInfo.environment) -> URL {
        if let override { return override }
        if let path = environment["HEAV3NS_NODE"], !path.isEmpty { return URL(fileURLWithPath: path) }
        return bundledNodeURL
    }
}
