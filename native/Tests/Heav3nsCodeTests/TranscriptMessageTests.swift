// [LAYER: UI]
import AppKit
import XCTest
@testable import Heav3nsCode

final class TranscriptMessageTests: XCTestCase {
    private final class TableFixture: NSObject, NSTableViewDataSource, NSTableViewDelegate {
        let row = TranscriptRow(id: "response", title: "LUMI · Completed", text: "Scripted UI fixture. The workspace contains a small Swift example for inspecting native navigation.", activity: false, evidence: "Checks not confirmed · Not reviewed")
        func numberOfRows(in tableView: NSTableView) -> Int { 1 }
        func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
            TranscriptMessageLayout(self.row, availableWidth: tableView.frameOfCell(atColumn: 0, row: row).width).height
        }
        func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
            TranscriptMessageCell(self.row, expanded: false, toggle: {}, openCheckpoint: {})
        }
    }
    func testTableInsetsAndRowMeasurementUseTheActualCellWidth() throws {
        _ = NSApplication.shared
        let fixture = TableFixture(), table = NSTableView(), scroll = NSScrollView()
        table.addTableColumn(NSTableColumn(identifier: .init("message")))
        table.delegate = fixture; table.dataSource = fixture; table.headerView = nil
        table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle; table.intercellSpacing = NSSize(width: 0, height: 20)
        scroll.documentView = table
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 700, height: 500), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false; window.contentView = scroll
        defer { window.close() }
        for style: NSTableView.Style in [.inset, .fullWidth] {
            table.style = style
            for width: CGFloat in [700, 320, 700] {
                window.setContentSize(NSSize(width: width, height: 500))
                window.contentView?.layoutSubtreeIfNeeded()
                table.reloadData(); table.noteHeightOfRows(withIndexesChanged: IndexSet(integer: 0)); table.layoutSubtreeIfNeeded()
                let cell = try XCTUnwrap(table.view(atColumn: 0, row: 0, makeIfNecessary: true) as? TranscriptMessageCell)
                cell.layoutSubtreeIfNeeded()
                XCTAssertTrue(cell.contentFits, "\(cell.layoutIssue ?? [:])")
            }
        }
    }
    func testLongMessagesKeepTheirMeasuredTextInsideTheRowAcrossResizes() {
        _ = NSApplication.shared
        let row = TranscriptRow(id: "recovery", title: "You", text: "Continue the saved work. Complete the original instruction and check the project, preserving existing changes.\n確認 العربية résumé 🌱", activity: false, user: true)
        let cell = TranscriptMessageCell(row, expanded: false, toggle: {}, openCheckpoint: {})
        let parent = NSView(frame: NSRect(x: 0, y: 0, width: 900, height: 2000))
        parent.addSubview(cell)
        for width: CGFloat in [320, 680, 270, 1000, 320, 680] {
            let geometry = TranscriptMessageLayout(row, availableWidth: width)
            cell.frame = NSRect(x: 0, y: 0, width: width, height: geometry.height)
            cell.layoutSubtreeIfNeeded()
            XCTAssertTrue(cell.contentFits, "Text escaped its row at width \(width)")
            let labels = cell.subviews.flatMap(\.subviews).compactMap { $0 as? NSTextField }.filter { !$0.isHidden }
            for field in labels {
                XCTAssertEqual(field.preferredMaxLayoutWidth, field.frame.width)
                let required = field.cell?.cellSize(forBounds: NSRect(x: 0, y: 0, width: field.frame.width, height: 10000)).height ?? 0
                XCTAssertLessThanOrEqual(required, field.frame.height + 1, "Native text measurement must fit")
            }
        }
    }
    func testActivityAndCheckpointControlsRetainNativeActionsAndSpace() throws {
        _ = NSApplication.shared
        let row = TranscriptRow(id: "detail", title: "A detailed activity title that wraps in a compact conversation", text: "Recorded detail", activity: true, checkpoint: "checkpoint", evidence: "Checks not confirmed")
        var toggles = 0, checkpoints = 0
        let cell = TranscriptMessageCell(row, expanded: false, toggle: { toggles += 1 }, openCheckpoint: { checkpoints += 1 })
        let geometry = TranscriptMessageLayout(row, availableWidth: 270)
        cell.frame = NSRect(x: 0, y: 0, width: 270, height: geometry.height)
        cell.layoutSubtreeIfNeeded()
        XCTAssertTrue(cell.contentFits)
        let buttons = cell.subviews.flatMap(\.subviews).compactMap { $0 as? NSButton }
        XCTAssertEqual(buttons.count, 2)
        buttons.forEach { $0.performClick(nil) }
        XCTAssertEqual(toggles, 1); XCTAssertEqual(checkpoints, 1)
        XCTAssertLessThan(try XCTUnwrap(geometry.body).maxY, try XCTUnwrap(geometry.checkpoint).minY)
    }
    func testSwarmWorkerBundlingChipDisplaysCorrectCountAndTitle() throws {
        let row = TranscriptRow(id: "run-1-activity", title: "Activity · 5 recorded [Swarm: 3 updates] · Subagent finished", text: "Details below", activity: true)
        XCTAssertTrue(row.title.contains("[Swarm: 3 updates]"))
        XCTAssertTrue(row.activity)
        let layout = TranscriptMessageLayout(row, availableWidth: 400)
        XCTAssertGreaterThan(layout.height, 20)
    }
    func testInlineWalkthroughActionButtonFitsAndDispatches() throws {
        _ = NSApplication.shared
        let row = TranscriptRow(id: "run-1-cmd", title: "Run test suite · in progress · 1.2s · Attempt 2 · Quiet 10s", text: "Running unit tests · No new command output for 10s.", activity: true, evidence: "src/app.swift", runID: "run-1", activityID: "cmd", actionTitle: "Open in Walkthrough")
        var opened = 0
        let cell = TranscriptMessageCell(row, expanded: true, toggle: {}, openCheckpoint: {}, openAction: { opened += 1 })
        for width: CGFloat in [270, 360, 680] {
            let geometry = TranscriptMessageLayout(row, availableWidth: width, expanded: true)
            cell.frame = NSRect(x: 0, y: 0, width: width, height: geometry.height)
            cell.layoutSubtreeIfNeeded()
            XCTAssertTrue(cell.contentFits, "\(cell.layoutIssue ?? [:])")
            XCTAssertLessThan(try XCTUnwrap(geometry.body).maxY, try XCTUnwrap(geometry.action).minY)
        }
        let buttons = cell.subviews.flatMap(\.subviews).compactMap { $0 as? NSButton }
        let openBtn = try XCTUnwrap(buttons.first { $0.title == "Open in Walkthrough" })
        openBtn.performClick(nil)
        XCTAssertEqual(opened, 1)
    }

    func testLiveHighlightedDiffCardFitsAndDispatchesViewDiff() throws {
        _ = NSApplication.shared
        let diff = """
        --- a/src/domain/item.rs
        +++ b/src/domain/item.rs
        @@ -1,2 +1,2 @@
        -pub struct Item { pub count: u32 }
        +pub struct Item { pub count: u64, pub active: bool }
        """
        let row = TranscriptRow(
            id: "run-1-diff-tools-r0",
            title: "Diff · src/domain/item.rs · (+1 / -1)",
            text: "",
            activity: true,
            runID: "run-1",
            activityID: "tools-r0",
            actionTitle: "Open in Walkthrough",
            diffText: diff,
            diffFile: "src/domain/item.rs",
            diffStats: "(+1 / -1)"
        )
        var openedDiff = 0
        let cell = TranscriptMessageCell(row, expanded: true, toggle: {}, openCheckpoint: {}, openAction: {}, openDiff: { openedDiff += 1 })
        for width: CGFloat in [270, 360, 680] {
            let geometry = TranscriptMessageLayout(row, availableWidth: width, expanded: true)
            cell.frame = NSRect(x: 0, y: 0, width: width, height: geometry.height)
            cell.layoutSubtreeIfNeeded()
            XCTAssertTrue(cell.contentFits, "\(cell.layoutIssue ?? [:])")
            XCTAssertNotNil(geometry.diffBox)
            XCTAssertNotNil(geometry.openDiff)
        }
        let buttons = cell.subviews.flatMap(\.subviews).compactMap { $0 as? NSButton }
        let viewDiffBtn = try XCTUnwrap(buttons.first { $0.title == "View Diff" })
        XCTAssertNotNil(buttons.first { $0.title == "Copy Diff" })
        viewDiffBtn.performClick(nil)
        XCTAssertEqual(openedDiff, 1)

        let longDiff = (1...30).map { "+\($0)" }.joined(separator: "\n")
        let bounded = CodeOutline.boundedDiffPreview(longDiff, maxLines: 18)
        XCTAssertEqual(bounded.omittedLines, 12)
        XCTAssertTrue(bounded.preview.contains("12 more line(s)"))
    }

    func testCodeOutlineMultiLineHunkAndWalkthroughDiffPreview() throws {
        let diff = """
        --- a/src/app.swift
        +++ b/src/app.swift
        @@ -10,2 +10,3 @@
        -let a = 1
        -let b = "old 🌱"
        +let a = 2
        +let b = "new 🌱"
        +let c = true
        """
        let summary = CodeOutline.diffSummary(diff)
        XCTAssertEqual(summary.added, 3)
        XCTAssertEqual(summary.removed, 2)
        XCTAssertEqual(summary.files, ["src/app.swift"])

        let highlighted = CodeOutline.highlight(diff, diff: true, fontSize: 11)
        XCTAssertEqual(highlighted.string, diff)

        let raw: [String: Any] = [
            "activityId": "tools-r1",
            "sequence": 4,
            "phase": "writing",
            "status": "completed",
            "message": "replace_file_content src/app.swift (+3/-2, 4ms)",
            "metadata": [
                "scope": "activity",
                "itemType": "tool",
                "operation": "execute",
                "files": ["src/app.swift"],
                "linesAdded": 3,
                "linesRemoved": 2
            ],
            "preview": [
                "kind": "diff",
                "text": diff,
                "limited": false
            ]
        ]
        let act = try XCTUnwrap(WalkthroughActivity(raw))
        XCTAssertEqual(act.preview?.kind, "diff")
        XCTAssertEqual(act.linesAdded, 3)
        XCTAssertEqual(act.linesRemoved, 2)
        XCTAssertEqual(act.previewTitle, "Live diff (+3 / -2)")
    }

    func testLiveStreamingReplyRowFitsAcrossWidths() {
        _ = NSApplication.shared
        let row = TranscriptRow(
            id: "r1-streaming-reply",
            title: "LUMI · Streaming…",
            text: "Inspecting the streaming pipeline and drafting the response incrementally.",
            activity: false,
            evidence: "Streaming response (1 KB)…"
        )
        let cell = TranscriptMessageCell(row, expanded: false, toggle: {}, openCheckpoint: {})
        for width: CGFloat in [270, 360, 680] {
            let geometry = TranscriptMessageLayout(row, availableWidth: width, expanded: true)
            cell.frame = NSRect(x: 0, y: 0, width: width, height: geometry.height)
            cell.layoutSubtreeIfNeeded()
            XCTAssertTrue(cell.contentFits, "Streaming reply escaped at width \(width): \(cell.layoutIssue ?? [:])")
        }
    }

    func testComposerCompressesToSlimHeightAfterClearingDraft() {
        _ = NSApplication.shared
        let composer = ComposerView(frame: NSRect(x: 0, y: 0, width: 520, height: 120))
        var invalidations = 0
        composer.onTextGeometryInvalidated = { invalidations += 1 }
        composer.string = "Line 1\nLine 2\nLine 3\nLine 4"
        XCTAssertEqual(invalidations, 1)
        composer.string = ""
        XCTAssertEqual(invalidations, 2)
        composer.dispose()
        XCTAssertNil(composer.onTextGeometryInvalidated)
    }

    func testCompressedDiffHighlightsOnlyChangesAndExpandsToFullLengthOnClick() throws {
        _ = NSApplication.shared
        var lines = [
            "diff --git a/src/app.swift b/src/app.swift",
            "--- a/src/app.swift",
            "+++ b/src/app.swift",
            "@@ -1,20 +1,20 @@ struct App {"
        ]
        for i in 1...12 { lines.append(" let unchanged\(i) = \(i)") }
        for i in 1...12 { lines.append("-\(i)_old()") ; lines.append("+\(i)_new()") }
        let fullDiff = lines.joined(separator: "\n")

        let compressed = CodeOutline.compressedDiffPreview(fullDiff, maxChangedLines: 8)
        XCTAssertEqual(compressed.totalChangedLines, 24)
        XCTAssertEqual(compressed.omittedLines, 16)
        XCTAssertFalse(compressed.preview.contains("unchanged1"))
        XCTAssertFalse(compressed.preview.contains("diff --git"))
        XCTAssertTrue(compressed.preview.contains("-1_old()"))
        XCTAssertTrue(compressed.preview.contains("+1_new()"))

        let row = TranscriptRow(id: "r1-diff-1", title: "Diff · src/app.swift · (+12 / -12)", text: "", activity: true, diffText: fullDiff, diffFile: "src/app.swift", diffStats: "(+12 / -12)")
        let collapsedLayout = TranscriptMessageLayout(row, availableWidth: 520, expanded: false)
        let expandedLayout = TranscriptMessageLayout(row, availableWidth: 520, expanded: true)
        XCTAssertGreaterThan(expandedLayout.height, collapsedLayout.height)

        var toggled = 0
        let cell = TranscriptMessageCell(row, expanded: false, toggle: { toggled += 1 }, openCheckpoint: {})
        cell.frame = NSRect(x: 0, y: 0, width: 520, height: collapsedLayout.height)
        cell.layoutSubtreeIfNeeded()
        XCTAssertTrue(cell.contentFits)
        let buttons = cell.subviews.flatMap(\.subviews).compactMap { $0 as? NSButton }
        let expandBtn = try XCTUnwrap(buttons.first { $0.title == "Expand Full Diff" })
        expandBtn.performClick(nil)
        XCTAssertEqual(toggled, 1)

        let liveRow = TranscriptRow(id: "r2-diff-live", title: "Streaming Diff · src/app.swift · (+2 / -1)", text: "", activity: true, diffText: "-old()\n+new1()\n+new2()", diffFile: "src/app.swift", diffStats: "(+2 / -1)", diffLive: true)
        let liveLayout = TranscriptMessageLayout(liveRow, availableWidth: 480, expanded: false)
        let liveCell = TranscriptMessageCell(liveRow, expanded: false, toggle: {}, openCheckpoint: {})
        liveCell.frame = NSRect(x: 0, y: 0, width: 480, height: liveLayout.height)
        liveCell.layoutSubtreeIfNeeded()
        XCTAssertTrue(liveCell.contentFits)
        liveCell.dispose()

        let activeAttr = CodeOutline.streamingDiffAttributed("-oldVal\n+newVal", hasMore: true, fontSize: 11)
        XCTAssertTrue(activeAttr.string.hasSuffix("▌"))
        let caretColor = activeAttr.attribute(.foregroundColor, at: activeAttr.length - 1, effectiveRange: nil) as? NSColor
        XCTAssertEqual(caretColor, NSColor.systemTeal)

        let settledAttr = CodeOutline.streamingDiffAttributed("-oldVal\n+newVal", hasMore: false, fontSize: 11)
        XCTAssertFalse(settledAttr.string.hasSuffix("▌"))
    }

    func testSourceControlWindowSplitViewFillsWindowHeightWithoutEmptyBottomGap() throws {
        _ = NSApplication.shared
        let sc = SourceControlWindow(folder: FileManager.default.temporaryDirectory) { _, _, completion in
            completion(.success(["entries": [], "isGitRepo": true, "branch": "main", "hasRemote": false, "ahead": 0, "behind": 0]))
        }
        defer { sc.close() }
        let contentView = try XCTUnwrap(sc.window?.contentView)
        contentView.layoutSubtreeIfNeeded()
        let split = try XCTUnwrap(contentView.subviews.first { $0 is NSSplitView })
        XCTAssertGreaterThan(split.frame.height, 540, "Split view must fill the window height instead of leaving empty space at the bottom")
    }

    func testSourceControlRemoteBarTouchlessGitHubStates() throws {
        _ = NSApplication.shared
        let status = GitHubRemoteStatus.parse([
            "ghInstalled": true,
            "ghAuthenticated": true,
            "ghUser": "CardSorting",
            "hasRemote": false,
            "webUrl": "",
            "remoteName": "",
            "repos": [
                ["nameWithOwner": "CardSorting/dragon", "url": "https://github.com/CardSorting/dragon", "isPrivate": true]
            ]
        ])
        XCTAssertTrue(status.ghAuthenticated)
        XCTAssertEqual(status.ghUser, "CardSorting")
        XCTAssertEqual(status.repos.count, 1)
        XCTAssertEqual(status.repos[0].nameWithOwner, "CardSorting/dragon")

        let bar = SourceControlRemoteBar(frame: NSRect(x: 0, y: 0, width: 700, height: 38))
        bar.update(status: status)
        bar.layoutSubtreeIfNeeded()
        bar.dispose()
    }
}
