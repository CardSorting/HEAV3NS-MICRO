// [LAYER: PLUMBING]
import AppKit
import XCTest
@testable import Heav3nsCode

final class PostureOmnibarReceiptsTests: XCTestCase {
    private func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }

    func testExecutionPostureFiveStopCyclingAndAliasParsing() {
        var stop: ExecutionPostureStop = .ask
        var visited = [ExecutionPostureStop]()
        for _ in 0..<5 {
            visited.append(stop)
            stop = stop.stepNext()
        }
        XCTAssertEqual(visited, [.ask, .acceptEdits, .plan, .autoReview, .fullAccess])
        XCTAssertEqual(stop, .ask, "Fifth Shift+Tab step must wrap back to Ask")

        XCTAssertEqual(ExecutionPostureStop.parse("plan"), .plan)
        XCTAssertEqual(ExecutionPostureStop.parse("accept_edits"), .acceptEdits)
        XCTAssertEqual(ExecutionPostureStop.parse("Auto-Review"), .autoReview)
        XCTAssertEqual(ExecutionPostureStop.parse("yolo"), .fullAccess)
        XCTAssertNil(ExecutionPostureStop.parse("nonexistent_mode"))
    }

    func testComposerShiftTabCyclesPostureWithoutBreakingMarkedTextOrTab() {
        _ = NSApplication.shared
        let composer = ComposerView(frame: NSRect(x: 0, y: 0, width: 320, height: 80))
        var stepped = 0
        composer.onStepPosture = { stepped += 1 }

        composer.doCommand(by: #selector(NSResponder.insertBacktab(_:)))
        XCTAssertEqual(stepped, 1)

        // IME marked text composition must not trigger posture cycling
        composer.setMarkedText("変換中", selectedRange: NSRange(location: 0, length: 3), replacementRange: NSRange(location: NSNotFound, length: 0))
        composer.doCommand(by: #selector(NSResponder.insertBacktab(_:)))
        XCTAssertEqual(stepped, 1)
        composer.unmarkText()
        composer.dispose()
    }

    func testTurnReceiptPresentationHonestBillingAndUndoSummaryBar() {
        _ = NSApplication.shared
        let rawReceipt: [String: Any] = [
            "schema": "heav3ns.receipt/v1",
            "turnId": "turn_42",
            "posture": "accept-edits",
            "durationMs": 420,
            "prefixStatus": "stable",
            "billingDisplay": "unknown",
            "snapshotBefore": "snap_1_aaaaaaaa",
            "snapshotAfter": "snap_1_bbbbbbbb",
            "fileChanges": [
                ["path": "src/main.rs", "kind": "modified", "linesAdded": 12, "linesRemoved": 3, "afterHash": "bbbbbbbb"]
            ]
        ]
        let pres = TurnReceiptPresentation(rawReceipt)
        XCTAssertNotNil(pres)
        XCTAssertEqual(pres?.filesChanged, 1)
        XCTAssertEqual(pres?.linesAdded, 12)
        XCTAssertEqual(pres?.linesRemoved, 3)
        XCTAssertEqual(pres?.cacheBadge, "KV stable")
        XCTAssertEqual(pres?.billingBadge, "unknown", "Unrecognized billing must never be fabricated as $0.00")
        XCTAssertTrue(pres?.hasReversibleChanges == true)

        let bar = TurnReceiptSummaryBar(frame: .zero)
        XCTAssertTrue(bar.isHidden)
        bar.update(with: pres)
        XCTAssertFalse(bar.isHidden)
        XCTAssertFalse(bar.undoButton.isHidden)
        XCTAssertFalse(bar.diffButton.isHidden)

        var undoTriggered = false
        bar.onUndoTurn = { undoTriggered = true }
        bar.undoButton.performClick(nil)
        XCTAssertTrue(undoTriggered)
        bar.dispose()
    }

    func testRequestPreviewFormatterRendersExactByteBreakdownAndPinnedPrefix() {
        let manifest: [String: Any] = [
            "schema": "heav3ns.request_manifest/v1",
            "model": "claude-sonnet-4-20250514",
            "posture": "plan",
            "prefixFingerprint": ["combinedHash": "8f4a2c10"],
            "prefixStatus": "pinned_initial",
            "systemBytes": 4200,
            "toolSchemaBytes": 6800,
            "historyBytes": 1200,
            "jsonFramingBytes": 140,
            "canonicalBodyBytes": 12340,
            "contextWindowTokens": 200000,
            "estimatedInputTokens": 3100,
            "headroomTokens": 196900,
            "activeTools": ["view_file", "grep_search", "tool_search"],
            "messageCount": 3
        ]
        let formatted = RequestPreviewFormatter.format(manifest)
        XCTAssertTrue(formatted.contains("heav3ns.request_manifest/v1"))
        XCTAssertTrue(formatted.contains("fnv1a:8f4a2c10 (pinned_initial)"))
        XCTAssertTrue(formatted.contains("12,340 exact bytes"))
        XCTAssertTrue(formatted.contains("view_file, grep_search, tool_search"))
    }

    func testApprovalReviewSupportsAllowForSessionAlongsideAllowOnceAndDeny() throws {
        _ = NSApplication.shared
        let req = ApprovalRequest(run: "run_1", operation: "apr_1", tool: "run_command", scope: "/tmp/ws", reason: "Network command", arguments: "cargo check")
        var recorded = [String]()
        let review = ApprovalReview(request: req, decide: { _, decision in recorded.append(decision) }, didClose: {})
        review.update(isCurrent: true, connected: true, submitting: false, stopping: false)
        let views = descendants(try XCTUnwrap(review.window?.contentView))
        let allowSession = try XCTUnwrap(views.compactMap { $0 as? NSButton }.first { $0.title == "Allow for Session" })
        XCTAssertTrue(allowSession.isEnabled)
        allowSession.performClick(nil)
        XCTAssertEqual(recorded, ["allow_session"])
        review.dispose()
    }

    func testOmnibarCommandPaletteWorksWithoutOpenFileAndUnsentAuthRestoresDraft() throws {
        _ = NSApplication.shared
        let folder = FileManager.default.temporaryDirectory.appendingPathComponent("heav3ns-ux-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: folder) }

        let workspace = WorkspaceWindow(folder: folder)
        defer { workspace.window?.close() }

        // Command palette (.actions) opens and lists Posture / Undo / Request Preview even before file-search is active
        workspace.showCommandPalette()
        XCTAssertEqual(workspace.quickOpen?.diagnostics["query"] as? String, ">")
        XCTAssertTrue((workspace.quickOpen?.diagnostics["count"] as? Int ?? 0) >= 8)
        workspace.quickOpen?.dispose()

        // Step posture via WorkspaceWindow
        XCTAssertEqual(workspace.activePosture, .autoReview)
        workspace.stepPosture()
        XCTAssertEqual(workspace.activePosture, .fullAccess)
        workspace.stepPosture()
        XCTAssertEqual(workspace.activePosture, .ask)

        // Simulate unsentAuthRejected run.state event -> rescues prompt back into composer!
        workspace.composer.string = ""
        workspace.receive([
            "kind": "event",
            "seq": 1,
            "type": "run.state",
            "payload": [
                "id": "run_auth_1",
                "text": "Refactor spatial engine carefully",
                "session": "",
                "state": "failed",
                "createdAt": 1000.0,
                "finishedAt": 1010.0,
                "unsentAuthRejected": true
            ]
        ])
        XCTAssertEqual(workspace.composer.string, "Refactor spatial engine carefully")
    }
}

