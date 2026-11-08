// [LAYER: INFRASTRUCTURE]
import AppKit
import Darwin
import Foundation
import Network
import Security
import zlib
#if canImport(FoundationModels)
import FoundationModels
#endif
#if canImport(ImagePlayground)
import ImagePlayground
#endif

/// Native macOS on-device LLM & Image Playground bridge ported from NOVAV2's Playground strategy.
/// Hosts Apple's on-device `FoundationModels` (`SystemLanguageModel` / `LanguageModelSession`)
/// and `ImagePlayground` (`ImageCreator` / `ImagePlaygroundViewController`) inside the app process
/// and exposes an authenticated loopback OpenAI-compatible `/v1` endpoint for the LUMI worker.
final class OnDevicePlaygroundService: @unchecked Sendable {
    static let shared = OnDevicePlaygroundService()
    static let providerID = "apple-playground"
    static let defaultModelID = "apple-foundation-model"

    struct ModelDescriptor: Sendable {
        let id: String
        let label: String
        let detail: String
        let contextLength: Int
        let tags: [String]
    }

    private let lock = NSLock()
    private let queue = DispatchQueue(label: "io.heav3ns.desktop.ondevice-playground", qos: .userInitiated, attributes: .concurrent)
    private var listener: NWListener?
    private var port: UInt16 = 0
    private let token: String

    init() {
        var bytes = [UInt8](repeating: 0, count: 24)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        self.token = bytes.map { String(format: "%02x", $0) }.joined()
    }

    func dispose() {
        lock.lock()
        listener?.cancel()
        listener = nil
        port = 0
        activeInferences = 0
        lock.unlock()
    }

    // MARK: - Device & Model Status (Ported from NOVAV2 NovaApp.swift)

    func sysctlString(_ name: String) -> String? {
        var size: size_t = 0
        guard sysctlbyname(name, nil, &size, nil, 0) == 0, size > 0 else { return nil }
        var buffer = [CChar](repeating: 0, count: size)
        guard sysctlbyname(name, &buffer, &size, nil, 0) == 0 else { return nil }
        return String(cString: buffer)
    }

    func deviceArchitecture() -> String {
        sysctlString("hw.machine") ?? "arm64"
    }

    func deviceMemoryBytes() -> UInt64 {
        ProcessInfo.processInfo.physicalMemory
    }

    func deviceMemoryGB() -> Int {
        Int((Double(deviceMemoryBytes()) / 1_073_741_824.0).rounded())
    }

    func deviceMacOSVersion() -> String {
        ProcessInfo.processInfo.operatingSystemVersionString
    }

    func localTextModelStatus() -> String {
        #if canImport(FoundationModels)
        guard #available(macOS 26.0, *) else { return "requires-macos-26" }
        switch SystemLanguageModel.default.availability {
        case .available:
            return "ready"
        case .unavailable(.modelNotReady):
            return "model-downloading-or-not-ready"
        case .unavailable(.appleIntelligenceNotEnabled):
            return "enable-apple-intelligence"
        case .unavailable(.deviceNotEligible):
            return "device-not-eligible"
        @unknown default:
            return "unavailable"
        }
        #else
        return "unavailable"
        #endif
    }

    func localImageModelReady() -> Bool {
        #if canImport(ImagePlayground)
        guard #available(macOS 15.4, *) else { return false }
        if Thread.isMainThread {
            return MainActor.assumeIsolated { ImagePlaygroundViewController.isAvailable }
        }
        return DispatchQueue.main.sync {
            MainActor.assumeIsolated { ImagePlaygroundViewController.isAvailable }
        }
        #else
        return false
        #endif
    }

    func statusSummary() -> String {
        let status = localTextModelStatus()
        let arch = deviceArchitecture()
        let memGB = deviceMemoryGB()
        let imgReady = localImageModelReady()
        let imgNote = imgReady ? " · Image Playground ready" : ""
        switch status {
        case "ready":
            return "Apple Intelligence ready on this Mac (\(arch), \(memGB) GB RAM)\(imgNote)."
        case "enable-apple-intelligence":
            return "Apple Intelligence is turned off. Enable Apple Intelligence in System Settings to run on-device models."
        case "model-downloading-or-not-ready":
            return "Apple on-device model is downloading or preparing in System Settings (\(arch), \(memGB) GB RAM)."
        case "device-not-eligible":
            return "This Mac (\(arch)) is not eligible for Apple Intelligence on-device FoundationModels."
        case "requires-macos-26":
            return "Apple on-device FoundationModels requires macOS 26 or newer."
        default:
            return "Apple on-device FoundationModels is currently unavailable on this Mac."
        }
    }

    func openAppleIntelligenceSettings() {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.Siri-Settings.extension") else { return }
        NSWorkspace.shared.open(url)
    }

    func availableModels() -> [ModelDescriptor] {
        let status = localTextModelStatus()
        let statusTag = status == "ready" ? "on-device, ready" : "on-device, \(status)"
        let memGB = deviceMemoryGB()
        var models = [
            ModelDescriptor(
                id: "apple-foundation-model",
                label: "Apple Intelligence (On-Device)",
                detail: "FoundationModels LanguageModelSession · \(memGB)GB unified memory · Private offline inference",
                contextLength: 4_096,
                tags: ["recommended", statusTag]
            ),
            ModelDescriptor(
                id: "apple-playground-coding",
                label: "Apple Playground Coding",
                detail: "On-device FoundationModels tuned for code synthesis, debugging, and tool execution",
                contextLength: 4_096,
                tags: ["coding", statusTag]
            ),
            ModelDescriptor(
                id: "apple-playground-fast",
                label: "Apple Playground Fast",
                detail: "Low-latency concise on-device FoundationModels session",
                contextLength: 4_096,
                tags: ["fast", statusTag]
            )
        ]
        if localImageModelReady() {
            models.append(
                ModelDescriptor(
                    id: "apple-image-playground",
                    label: "Apple Image Playground",
                    detail: "On-device ImagePlayground ImageCreator & FoundationModels multimodal assistant",
                    contextLength: 4_096,
                    tags: ["vision", "image-playground", "on-device"]
                )
            )
        } else {
            models.append(
                ModelDescriptor(
                    id: "apple-image-playground",
                    label: "Apple Image Playground",
                    detail: "On-device ImagePlayground (requires Apple Intelligence Image Playground enabled)",
                    contextLength: 4_096,
                    tags: ["image-playground", statusTag]
                )
            )
        }
        return models
    }

    // MARK: - Child Worker Environment Variables (Ported from NOVAV2 + HEAV3NS keys)

    func workerEnvironment() -> [String: String] {
        let endpoint = ensureServerStarted()
        let textStatus = localTextModelStatus()
        let textReady = textStatus == "ready" ? "1" : "0"
        let imageReady = localImageModelReady() ? "1" : "0"
        let arch = deviceArchitecture()
        let memory = String(deviceMemoryBytes())
        let macos = deviceMacOSVersion()
        var env: [String: String] = [
            "HEAV3NS_DEVICE_ARCH": arch,
            "HEAV3NS_DEVICE_MEMORY_BYTES": memory,
            "HEAV3NS_DEVICE_MACOS": macos,
            "HEAV3NS_LOCAL_TEXT_STATUS": textStatus,
            "HEAV3NS_LOCAL_TEXT_READY": textReady,
            "HEAV3NS_LOCAL_IMAGE_READY": imageReady,
            "NOVA_DEVICE_ARCH": arch,
            "NOVA_DEVICE_MEMORY_BYTES": memory,
            "NOVA_DEVICE_MACOS": macos,
            "NOVA_LOCAL_TEXT_STATUS": textStatus,
            "NOVA_LOCAL_TEXT_READY": textReady,
            "NOVA_LOCAL_IMAGE_READY": imageReady
        ]
        if let endpoint {
            env["HEAV3NS_LOCAL_MODEL_URL"] = endpoint.baseURL
            env["HEAV3NS_LOCAL_MODEL_TOKEN"] = endpoint.token
        }
        return env
    }

    // MARK: - Local Loopback OpenAI-Compatible Bridge Server

    @discardableResult
    func ensureServerStarted() -> (baseURL: String, token: String)? {
        lock.lock()
        if let listener, listener.state == .ready, port > 0 {
            let currentPort = port
            lock.unlock()
            return ("http://127.0.0.1:\(currentPort)/v1", token)
        }
        lock.unlock()

        let params = NWParameters.tcp
        params.acceptLocalOnly = true
        params.requiredLocalEndpoint = NWEndpoint.hostPort(host: .ipv4(.loopback), port: .any)

        guard let newListener = try? NWListener(using: params) else { return nil }
        let readySemaphore = DispatchSemaphore(value: 0)
        final class ReadyFlag: @unchecked Sendable { var signaled = false }
        let flag = ReadyFlag()

        newListener.stateUpdateHandler = { [weak self] state in
            guard let self else { return }
            switch state {
            case .ready:
                if let assigned = newListener.port?.rawValue, assigned > 0 {
                    self.lock.lock()
                    self.port = assigned
                    self.lock.unlock()
                }
                if !flag.signaled {
                    flag.signaled = true
                    readySemaphore.signal()
                }
            case .failed, .cancelled:
                self.lock.lock()
                if self.listener === newListener { self.listener = nil; self.port = 0 }
                self.lock.unlock()
                if !flag.signaled {
                    flag.signaled = true
                    readySemaphore.signal()
                }
            default:
                break
            }
        }

        newListener.newConnectionHandler = { [weak self] connection in
            self?.handleConnection(connection)
        }

        newListener.start(queue: queue)
        _ = readySemaphore.wait(timeout: .now() + 2.0)

        lock.lock()
        let boundPort = self.port
        if boundPort > 0 {
            self.listener = newListener
        }
        lock.unlock()

        guard boundPort > 0 else {
            newListener.cancel()
            return nil
        }
        return ("http://127.0.0.1:\(boundPort)/v1", token)
    }

    private func handleConnection(_ connection: NWConnection) {
        connection.start(queue: queue)
        receiveHTTPRequest(on: connection, accumulated: Data())
    }

    private func receiveHTTPRequest(on connection: NWConnection, accumulated: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, isComplete, error in
            guard let self else { connection.cancel(); return }
            if error != nil {
                connection.cancel()
                return
            }
            var buffer = accumulated
            if let data, !data.isEmpty {
                buffer.append(data)
            }
            if buffer.count > Self.maxRequestBytes || (Self.headerEnd(in: buffer) == nil && buffer.count > Self.maxHeaderBytes) {
                self.sendJSONResponse(on: connection, status: 413, body: ["error": ["message": "Payload too large", "code": "payload_too_large"]])
                return
            }
            if let parsed = Self.parseHTTPRequest(from: buffer) {
                self.dispatchHTTPRequest(parsed, on: connection)
                return
            }
            if isComplete {
                connection.cancel()
                return
            }
            self.receiveHTTPRequest(on: connection, accumulated: buffer)
        }
    }

    static let maxRequestBytes = 8 * 1024 * 1024
    static let maxHeaderBytes = 32 * 1024
    static let maxConcurrentInferences = 2
    private var activeInferences = 0

    static func headerEnd(in data: Data) -> Range<Data.Index>? {
        data.range(of: Data([13, 10, 13, 10]))
    }

    func beginInference() -> Bool {
        lock.lock(); defer { lock.unlock() }
        guard activeInferences < Self.maxConcurrentInferences else { return false }
        activeInferences += 1
        return true
    }

    func endInference() {
        lock.lock(); activeInferences = max(0, activeInferences - 1); lock.unlock()
    }

    struct HTTPRequest {
        let method: String
        let path: String
        let headers: [String: String]
        let body: Data
    }

    static func parseHTTPRequest(from data: Data) -> HTTPRequest? {
        let separator = Data([13, 10, 13, 10])
        guard let headerRange = data.range(of: separator) else { return nil }
        let headerData = data.subdata(in: 0..<headerRange.lowerBound)
        guard let headerText = String(data: headerData, encoding: .utf8) else { return nil }
        let lines = headerText.components(separatedBy: "\r\n")
        guard let requestLine = lines.first else { return nil }
        let parts = requestLine.split(separator: " ", maxSplits: 2).map(String.init)
        guard parts.count >= 2 else { return nil }
        let method = parts[0].uppercased()
        let path = parts[1]
        var headers = [String: String]()
        for line in lines.dropFirst() {
            guard let colon = line.firstIndex(of: ":") else { continue }
            let key = line[..<colon].trimmingCharacters(in: .whitespaces).lowercased()
            let value = line[line.index(after: colon)...].trimmingCharacters(in: .whitespaces)
            headers[key] = value
        }
        let bodyStart = headerRange.upperBound
        guard let contentLength = Int(headers["content-length"] ?? "0"), contentLength >= 0, contentLength <= maxRequestBytes else { return nil }
        guard data.count - bodyStart >= contentLength else { return nil }
        let body = contentLength > 0 ? data.subdata(in: bodyStart..<(bodyStart + contentLength)) : Data()
        return HTTPRequest(method: method, path: path, headers: headers, body: body)
    }

    private func isAuthorized(_ headers: [String: String]) -> Bool {
        guard let auth = headers["authorization"] else { return false }
        let expected = Array("Bearer \(token)".utf8), provided = Array(auth.utf8)
        guard expected.count == provided.count else { return false }
        var difference: UInt8 = 0
        for (a, b) in zip(expected, provided) { difference |= a ^ b }
        return difference == 0
    }

    private func dispatchHTTPRequest(_ request: HTTPRequest, on connection: NWConnection) {
        if !isAuthorized(request.headers) {
            sendJSONResponse(on: connection, status: 401, body: ["error": ["message": "Unauthorized local model request", "code": "unauthorized"]])
            return
        }

        let cleanPath = request.path.split(separator: "?", maxSplits: 1).first.map(String.init) ?? request.path
        switch (request.method, cleanPath) {
        case ("GET", "/v1/status"), ("GET", "/status"):
            sendJSONResponse(on: connection, status: 200, body: [
                "status": localTextModelStatus(),
                "textReady": localTextModelStatus() == "ready",
                "imageReady": localImageModelReady(),
                "arch": deviceArchitecture(),
                "memoryBytes": deviceMemoryBytes(),
                "macOS": deviceMacOSVersion(),
                "summary": statusSummary()
            ])
        case ("GET", "/v1/models"), ("GET", "/models"):
            let list = availableModels().map { model in
                [
                    "id": model.id,
                    "object": "model",
                    "owned_by": "apple-on-device",
                    "name": model.label,
                    "description": model.detail,
                    "context_length": model.contextLength,
                    "tags": model.tags
                ] as [String: Any]
            }
            sendJSONResponse(on: connection, status: 200, body: [
                "object": "list",
                "data": list
            ])
        case ("POST", "/v1/chat/completions"), ("POST", "/chat/completions"):
            let inference = Task {
                await self.handleChatCompletions(request, on: connection)
            }
            connection.stateUpdateHandler = { state in
                switch state {
                case .failed, .cancelled: inference.cancel()
                default: break
                }
            }
        default:
            sendJSONResponse(on: connection, status: 404, body: ["error": ["message": "Not found: \(cleanPath)", "code": "not_found"]])
        }
    }
}
