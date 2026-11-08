// [LAYER: INFRASTRUCTURE]
import XCTest
import AppKit
@testable import Heav3nsCode

class RuntimeClientTestCase: XCTestCase {
    var directory: URL!
    var clients = [RuntimeClient]()

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory.appendingPathComponent("heav3ns-client-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        for client in clients { client.forceStop() }
        clients.removeAll()
        if let dir = directory {
            try? FileManager.default.removeItem(at: dir)
        }
    }

    func peer(_ behavior: String, timeout: TimeInterval = 0.3, nodeOverride: URL? = nil, sleepAssertion: RunSleepAssertion = RunSleepAssertion()) throws -> RuntimeClient {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { root.deleteLastPathComponent() }
        let bundledNode = root.appendingPathComponent("build/HEAV3NS-CODE.app/Contents/Resources/runtime/node")
        let candidateNodes = [
            nodeOverride,
            bundledNode,
            URL(fileURLWithPath: "/opt/homebrew/bin/node"),
            URL(fileURLWithPath: "/usr/local/bin/node"),
            URL(fileURLWithPath: "/usr/bin/node")
        ].compactMap { $0 }

        guard let node = candidateNodes.first(where: { FileManager.default.isExecutableFile(atPath: $0.path) }) else {
            throw XCTSkip("Build the desktop package to test its pinned Node runtime.")
        }
        let script = directory.appendingPathComponent("peer-\(UUID().uuidString).cjs")
        let source = """
        const send = (value) => process.stdout.write(JSON.stringify({ v: 1, ...value }) + '\\n');
        const reply = (request, result) => send({ kind: 'response', id: request.id, result });
        let buffer = '';
        process.stdin.on('end', () => process.exit(0));
        process.stdin.on('data', chunk => {
          buffer += chunk;
          for (;;) {
            const end = buffer.indexOf('\\n'); if (end < 0) break;
            const request = JSON.parse(buffer.slice(0, end)); buffer = buffer.slice(end + 1);
            \(behavior)
            if (request.method === 'agentSwarm.settings') { reply(request, { enabled: false, criticalCompletionReview: false, autoDeferOnCompletionReview: true, hierarchicalDefer: false, legion: false, deferBoundary: 'safe_boundary', strategy: 'adaptive', maxSpecialists: 4, environmentOverrides: [] }); continue; }
            if (request.method === 'task.deferStatus') { reply(request, { enabled: false, deferred: false, queued: 0 }); continue; }
          }
        });
        """
        try source.write(to: script, atomically: true, encoding: .utf8)
        let client = RuntimeClient(node: node, cli: script, requestTimeout: timeout, sleepAssertion: sleepAssertion)
        clients.append(client)
        return client
    }

    let hello = "if (request.method === 'hello') { reply(request, { protocol: 1, workspace: 'ws', session: 'session', runtime: 'runtime' }); continue; }"
    let snapshot = "if (request.method === 'snapshot') { reply(request, { workspace: 'ws', session: 'session', runtime: 'runtime', seq: 0, runs: [] }); continue; }"
}

