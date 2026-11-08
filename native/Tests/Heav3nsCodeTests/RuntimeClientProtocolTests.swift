// [LAYER: INFRASTRUCTURE]
import XCTest
import AppKit
@testable import Heav3nsCode

final class RuntimeClientProtocolTests: RuntimeClientTestCase {
    func testInspectionHistoryRejectsLateReadsAndRecoversFromMissingContent() throws {
        _ = NSApplication.shared
        let behavior = """
        if (request.method === 'hello') { reply(request, { protocol: 1, workspace: 'ws', session: 'session', runtime: 'runtime', capabilities: ['run-details'] }); continue; }
        \(snapshot)
        if (request.method === 'files.list') reply(request, { entries: [] });
        if (request.method === 'files.read') {
          if (request.params.path === 'missing') send({ kind: 'response', id: request.id, error: { message: 'FILE_MISSING: choose another file or reload' } });
          else setTimeout(() => reply(request, { text: 'older file content', revision: 'file', nextOffset: 18, size: 18, more: false }), 100);
        }
        if (request.method === 'history.read') reply(request, { run: { id: request.params.run, text: 'Preserved cancelled instruction', state: 'cancelled', session: 'session', createdAt: 1, verification: 'not_run' }, outputAvailable: false, outputMessage: 'No output was recorded for this run.' });
        """
        let client = try peer(behavior, timeout: 1)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        defer { client.onEvent = nil; client.onConnection = nil; workspace.window?.close() }
        let checked = expectation(description: "late replies cannot replace run details or missing-content recovery")
        let connection = client.onConnection
        var requested = false
        client.onConnection = { value in
            connection?(value)
            guard client.isReady, !requested else { return }; requested = true
            workspace.inspect(.file("slow")); workspace.inspect(.run("cancelled"))
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
                XCTAssertTrue((workspace.probeLayout["inspectionPreview"] as? String ?? "").contains("Preserved cancelled instruction"))
                XCTAssertEqual(workspace.probeLayout["recordedOutputAvailable"] as? Bool, false)
                XCTAssertTrue(workspace.canInspectBack)
                workspace.inspect(.file("missing"))
                client.request("snapshot") { _ in
                    XCTAssertTrue((workspace.probeLayout["inspectionPreview"] as? String ?? "").contains("FILE_MISSING"))
                    XCTAssertEqual(workspace.probeLayout["inspectionPage"] as? String, "Could not load · use Reload")
                    workspace.inspectBack()
                    client.request("snapshot") { _ in
                        XCTAssertEqual(workspace.probeLayout["inspectionTitle"] as? String, "Run Details")
                        XCTAssertTrue(workspace.canInspectForward)
                        XCTAssertTrue((workspace.probeLayout["inspectionPreview"] as? String ?? "").contains("Preserved cancelled instruction"))
                        checked.fulfill()
                    }
                }
            }
        }
        workspace.attach(); wait(for: [checked], timeout: 4)
    }

    func testHandshakeDeadlineReleasesCallbacksAndClosesThePrivatePipe() throws {
        let client = try peer("/* Deliberately never acknowledge hello. */")
        let ended = expectation(description: "EOF cleans up the peer")
        client.onExit = { ended.fulfill() }
        client.start(folder: directory)
        wait(for: [ended], timeout: 3)
        XCTAssertFalse(client.isReady); XCTAssertEqual(client.pendingRequestCount, 0)
    }

    func testLostSubmissionAcknowledgmentIsUnknownAndNeverAutomaticallyRetried() throws {
        let client = try peer(hello + snapshot + "if (request.method === 'task.submit') { /* Lost acknowledgement. */ }")
        let failed = expectation(description: "uncertain acceptance reported once")
        failed.assertForOverFulfill = true
        client.onConnection = { _ in
            if client.isReady {
                client.request("task.submit", params: ["text": "fixture"], id: "same-request") { result in
                    guard case .failure(let error) = result else { return XCTFail("Submission should time out") }
                    XCTAssertEqual((error as? RuntimeFailure)?.acceptanceUnknown, true)
                    XCTAssertEqual(client.pendingRequestCount, 0)
                    failed.fulfill()
                }
            }
        }
        client.start(folder: directory)
        wait(for: [failed], timeout: 3)
        XCTAssertFalse(client.isReady)
    }

    func testReadDeadlinePreservesTheDistinctionFromUnknownAcceptance() throws {
        let client = try peer(hello + snapshot)
        let failed = expectation(description: "read request expires")
        client.onConnection = { _ in
            if client.isReady { client.request("files.read", params: ["path": "fixture"]) { result in
                guard case .failure(let error) = result else { return XCTFail("Read should time out") }
                XCTAssertEqual((error as? RuntimeFailure)?.acceptanceUnknown, false); failed.fulfill()
            } }
        }
        client.start(folder: directory); wait(for: [failed], timeout: 3)
    }

    func testMalformedAcceptanceCannotClearAnUnconfirmedInstruction() throws {
        let client = try peer(hello + snapshot + "reply(request, { accepted: true });")
        let rejected = expectation(description: "invalid acceptance is unknown")
        client.onConnection = { _ in
            if client.isReady { client.request("task.submit", params: ["text": "fixture"]) { result in
                guard case .failure(let error) = result else { return XCTFail("Malformed acceptance must not succeed") }
                XCTAssertEqual((error as? RuntimeFailure)?.acceptanceUnknown, true); rejected.fulfill()
            } }
        }
        client.start(folder: directory); wait(for: [rejected], timeout: 3)
        XCTAssertFalse(client.isReady)
    }

    func testLateReadResponseDoesNotDisconnectActiveWork() throws {
        let client = try peer(hello + snapshot + """
        if (request.method === 'files.read') setTimeout(() => reply(request, { intentionally: 'late and invalid' }), 450);
        if (request.method === 'task.cancelAll') reply(request, { requested: true });
        """)
        let checked = expectation(description: "late inspection response is ignored and stop remains available")
        client.onConnection = { _ in
            guard client.isReady else { return }
            client.request("files.read", params: ["path": "slow"]) { result in
                guard case .failure(let error) = result else { return XCTFail("Slow read must expire") }
                XCTAssertEqual((error as? RuntimeFailure)?.acceptanceUnknown, false)
                XCTAssertTrue(client.isReady)
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                    client.request("task.cancelAll") { result in
                        if case .failure(let error) = result { XCTFail(error.localizedDescription) }
                        XCTAssertTrue(client.isReady); checked.fulfill()
                    }
                }
            }
        }
        client.start(folder: directory); wait(for: [checked], timeout: 3)
    }

    func testInspectionSaturationReservesControlCapacity() throws {
        let client = try peer(hello + snapshot + "if (request.method === 'task.cancelAll') reply(request, { requested: true });", timeout: 1)
        let stopped = expectation(description: "control reaches the worker before blocked reads finish")
        client.onConnection = { _ in
            guard client.isReady else { return }
            for _ in 0..<56 { client.request("files.read", params: ["path": "blocked"]) }
            client.request("files.list") { result in
                guard case .failure = result else { return XCTFail("Ordinary admission should be bounded") }
            }
            client.request("task.cancelAll") { result in
                if case .failure(let error) = result { XCTFail(error.localizedDescription) }
                XCTAssertTrue(client.isReady); XCTAssertEqual(client.pendingRequestCount, 56)
                client.requestSnapshot { XCTAssertTrue(client.isReady); stopped.fulfill() }
            }
        }
        client.start(folder: directory); wait(for: [stopped], timeout: 3)
    }

    func testMemoryContextSpiderAndStateReadTimeoutsNeverDisconnectRunningWork() throws {
        let client = try peer(hello + snapshot + """
        if (['memory.list','memory.read','context.inspect','context.history','spider.check','state.inspect'].includes(request.method)) setTimeout(() => reply(request,{late:true}),450);
        if (request.method === 'task.cancelAll') reply(request,{requested:true});
        """)
        let expired = expectation(description: "all context inspections time out locally"); expired.expectedFulfillmentCount = 6
        let stopped = expectation(description: "Stop still works after late replies")
        var started = false
        client.onConnection = { _ in
            guard client.isReady, !started else { return }; started = true
            for method in ["memory.list", "memory.read", "context.inspect", "context.history", "spider.check", "state.inspect"] {
                client.request(method) { result in
                    guard case .failure(let error) = result else { return XCTFail("Read should time out") }
                    XCTAssertEqual((error as? RuntimeFailure)?.acceptanceUnknown, false); XCTAssertTrue(client.isReady)
                    expired.fulfill()
                }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.7) {
                client.request("task.cancelAll") { result in
                    if case .failure(let error) = result { XCTFail(error.localizedDescription) }
                    XCTAssertTrue(client.isReady); stopped.fulfill()
                }
            }
        }
        client.start(folder: directory); wait(for: [expired, stopped], timeout: 3)
    }

    func testDiscoverySaturationLeavesRoomToSendInspectAndStop() throws {
        let client = try peer(hello + snapshot + """
        if (request.method === 'task.lookup') reply(request, { found: false });
        if (request.method === 'files.read') reply(request, { text: 'Still responsive', revision: 'r', nextOffset: 16, size: 16 });
        if (request.method === 'task.cancelAll') reply(request, { requested: true });
        if (request.method === 'task.submit') reply(request, { accepted: true, run: { id: 'accepted', text: request.params.text, state: 'queued', session: 'session', createdAt: 1 } });
        """, timeout: 2)
        let responsive = expectation(description: "interactive requests keep admission")
        responsive.expectedFulfillmentCount = 4
        var started = false
        client.onConnection = { _ in
            guard client.isReady, !started else { return }; started = true
            for _ in 0..<40 { client.request("models.discover") }
            XCTAssertEqual(client.pendingRequestCount, 40)
            client.request("files.search") { result in
                guard case .failure(let error) = result else { return XCTFail("Discovery should be bounded") }
                XCTAssertTrue(error.localizedDescription.contains("Discovery is busy"))
                XCTAssertEqual((error as? RuntimeFailure)?.acceptanceUnknown, false)
            }
            for method in ["task.lookup", "files.read", "task.submit", "task.cancelAll"] {
                client.request(method, params: ["text": "Continue independently"]) { result in
                    if case .failure(let error) = result { XCTFail(error.localizedDescription) }
                    XCTAssertTrue(client.isReady); responsive.fulfill()
                }
            }
        }
        client.start(folder: directory); wait(for: [responsive], timeout: 3)
    }

    func testSubmissionLookupTimeoutDoesNotInterruptWorkOrReplaySubmission() throws {
        let client = try peer(hello + snapshot + """
        if (request.method === 'task.lookup') setTimeout(() => reply(request, { found: false }), 450);
        if (request.method === 'task.cancelAll') reply(request, { requested: true });
        if (request.method === 'task.submit') process.exit(99);
        """)
        let checked = expectation(description: "lookup can be retried without disconnecting active work")
        client.onConnection = { _ in
            guard client.isReady else { return }
            client.request("task.lookup", params: ["request": "unconfirmed"]) { result in
                guard case .failure(let error) = result else { return XCTFail("Lookup must time out") }
                XCTAssertEqual((error as? RuntimeFailure)?.acceptanceUnknown, false)
                XCTAssertTrue(client.isReady)
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                    client.request("task.cancelAll") { result in
                        if case .failure(let error) = result { XCTFail(error.localizedDescription) }
                        XCTAssertTrue(client.isReady); checked.fulfill()
                    }
                }
            }
        }
        client.start(folder: directory); wait(for: [checked], timeout: 3)
    }

    func testWorkspaceOffersStatusRetryAfterLookupTimeoutWithoutLosingDraft() throws {
        _ = NSApplication.shared
        let drafts = WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("retry-drafts"))
        try drafts.save(text: "My newer draft", submission: .init(id: "unconfirmed", text: "Original instruction"))
        let client = try peer(hello + snapshot + """
        if (request.method === 'files.list') reply(request, { entries: [] });
        if (request.method === 'task.lookup') {
          globalThis.lookups = (globalThis.lookups || 0) + 1;
          if (globalThis.lookups > 1) setTimeout(() => reply(request, { found: false }), 40);
        }
        if (request.method === 'task.submit') process.exit(99);
        """, timeout: 0.2)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: drafts)
        defer { client.onConnection = nil; client.forceStop(); workspace.window?.close() }
        let checked = expectation(description: "status retry recovers without replay")
        let connection = client.onConnection
        client.onConnection = { value in
            connection?(value)
            guard client.isReady else { return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
                let views = descendants(workspace.window!.contentView!)
                let retry = views.compactMap { $0 as? NSButton }.first { $0.title == "Check Status" }
                XCTAssertNotNil(retry); XCTAssertTrue(retry?.isEnabled == true); XCTAssertFalse(retry?.isHidden == true)
                XCTAssertTrue(client.isReady)
                retry?.performClick(nil); retry?.performClick(nil)
                XCTAssertFalse(retry?.isEnabled == true, "Only one status lookup may be pending")
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
                    XCTAssertNil(try? drafts.load()?.submission)
                    XCTAssertEqual(try? drafts.load()?.text, "My newer draft")
                    XCTAssertEqual(workspace.probeLayout["submissionNeedsReconciliation"] as? Bool, false)
                    XCTAssertTrue(client.isReady); checked.fulfill()
                }
            }
        }
        workspace.attach(); wait(for: [checked], timeout: 3)
    }

    func testCoalescedSnapshotsCompleteEveryWaitingCaller() throws {
        let client = try peer(hello + "if (request.method === 'snapshot') setTimeout(() => reply(request, { workspace: 'ws', session: 'session', runtime: 'runtime', seq: 0, runs: [] }), 50);", timeout: 1)
        let completed = expectation(description: "both snapshot callers finish"); completed.expectedFulfillmentCount = 2
        client.onConnection = { _ in
            guard client.isReady else { return }
            client.requestSnapshot { completed.fulfill() }
            client.requestSnapshot { completed.fulfill() }
        }
        client.start(folder: directory); wait(for: [completed], timeout: 3)
    }

    func testSnapshotFromAnotherWorkspaceDisablesExecution() throws {
        let client = try peer(hello + "reply(request, { workspace: 'other', session: 'session', runtime: 'runtime', runs: [] });")
        let ended = expectation(description: "mismatch stops connection")
        client.onExit = { ended.fulfill() }
        client.start(folder: directory); wait(for: [ended], timeout: 3)
        XCTAssertFalse(client.isReady); XCTAssertEqual(client.pendingRequestCount, 0)
    }

    func testBlockedWriterDoesNotBlockIncomingEventsOrMainThreadDeadlines() throws {
        let behavior = hello + """
        if (request.method === 'snapshot') {
          reply(request, { workspace: 'ws', session: 'session', runtime: 'runtime', seq: 0, runs: [] });
          process.stdin.pause();
          setInterval(() => send({ kind: 'event', workspace: 'ws', session: 'session', runtime: 'runtime', type: 'fixture', payload: {} }), 10);
          continue;
        }
        """
        let client = try peer(behavior, timeout: 0.5)
        let received = expectation(description: "reads drain while writes back up")
        let expired = expectation(description: "deadline remains responsive")
        var reported = false, seen = false, lastConnection = "Not started"
        client.onEvent = { event in if event["type"] as? String == "fixture", !seen { seen = true; received.fulfill() } }
        client.onConnection = { value in
            lastConnection = value
            if client.isReady {
                for index in 0..<56 {
                    client.request("fixture", params: ["data": String(repeating: "x", count: 60_000)], id: "request-\(index)") { result in
                        if case .failure = result, !reported { reported = true; expired.fulfill() }
                    }
                }
            }
        }
        client.start(folder: directory); wait(for: [received, expired], timeout: 3)
        XCTAssertTrue(seen, lastConnection)
        XCTAssertEqual(client.pendingRequestCount, 0)
    }

    func testOldStopDeadlineCannotEscalateAReplacementRun() throws {
        _ = NSApplication.shared
        let behavior = hello + snapshot + """
        globalThis.runs ??= []; globalThis.seq ??= 0;
        const event = run => send({ kind: 'event', workspace: 'ws', session: 'session', runtime: 'runtime', run: run.id, type: 'run.state', seq: ++globalThis.seq, payload: run });
        if (request.method === 'task.submit') {
          const run = { id: 'run-' + globalThis.runs.length, request: request.id, session: 'session', text: request.params.text, createdAt: Date.now(), state: 'running', activities: [] };
          globalThis.runs.push(run); reply(request, { accepted: true, run }); event(run);
        } else if (request.method === 'task.cancel') {
          const run = globalThis.runs.find(run => run.id === request.params.run); run.state = 'cancelled'; event(run); reply(request, { requested: true });
        } else if (request.method === 'files.list') { reply(request, { entries: [] }); }
        """
        let client = try peer(behavior, timeout: 2)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        defer { client.onEvent = nil; client.onConnection = nil; workspace.window?.close() }
        let checked = expectation(description: "old cancellation deadline cannot change the next run")
        let originalConnection = client.onConnection, originalEvent = client.onEvent
        var sent = false, replaced = false, lastConnection = "Not started"
        client.onConnection = { value in
            lastConnection = value
            originalConnection?(value)
            if client.isReady && !sent { sent = true; workspace.submitProbe("first") }
        }
        client.onEvent = { event in
            originalEvent?(event)
            guard let payload = event["payload"] as? [String: Any] else { return }
            if payload["text"] as? String == "first", payload["state"] as? String == "running" { workspace.stop() }
            if payload["text"] as? String == "first", payload["state"] as? String == "cancelled", !replaced {
                replaced = true; workspace.submitProbe("replacement")
                DispatchQueue.main.asyncAfter(deadline: .now() + 5.2) {
                    XCTAssertTrue(workspace.hasWork)
                    XCTAssertTrue(client.isReady, lastConnection)
                    XCTAssertEqual(workspace.probeLayout["forceStopAvailable"] as? Bool, false)
                    checked.fulfill()
                }
            }
        }
        workspace.attach(); wait(for: [checked], timeout: 9)
    }

    func testChangedApprovalArgumentsInvalidateTheSheetAndLateFailureCannotClearANewerDecision() throws {
        _ = NSApplication.shared
        let client = try peer(hello + """
        globalThis.run ??= {id:'review',text:'Review',state:'awaiting_approval',session:'session',createdAt:1,revision:1,approval:{operation:'same',tool:'fixture',scope:'/workspace',args:'Original',reason:'Review'}};
        globalThis.decisions ??= 0;
        if (request.method === 'snapshot') reply(request,{workspace:'ws',session:'session',runtime:'runtime',seq:globalThis.changed ? 1 : 0,runs:[globalThis.run]});
        if (request.method === 'files.list') reply(request,{entries:[]});
        if (request.method === 'approval.resolve') {
          globalThis.decisions++;
          if (globalThis.decisions === 1) {
            setTimeout(() => {
              globalThis.changed=true; globalThis.run.revision++; globalThis.run.approval.args='Replacement';
              send({kind:'event',workspace:'ws',session:'session',runtime:'runtime',run:'review',type:'approval.requested',seq:1,payload:globalThis.run});
            },20);
            setTimeout(() => send({kind:'response',id:request.id,error:{message:'Late rejection of original'}}),80);
          }
        }
        if (request.method === 'history.read') reply(request,{run:{...globalThis.run,text:String(globalThis.decisions)},outputAvailable:false});
        """, timeout: 2)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        defer {
            if let parent = workspace.window, let sheet = parent.attachedSheet { parent.endSheet(sheet); sheet.orderOut(nil) }
            client.onEvent = nil; client.onConnection = nil; client.forceStop(); workspace.window?.close()
        }
        let checked = expectation(description: "replacement decision survives late original failure")
        let connection = client.onConnection, event = client.onEvent
        var original: ApprovalRequest?, allow: NSButton?
        client.onConnection = { value in
            connection?(value)
            guard client.isReady, original == nil else { return }
            original = workspace.podSnapshot.approval
            workspace.showApprovalReview()
            func views(_ parent: NSView) -> [NSView] { [parent] + parent.subviews.flatMap(views) }
            allow = workspace.window?.attachedSheet?.contentView.map(views)?.compactMap { $0 as? NSButton }.first { $0.title == "Allow Once" }
            XCTAssertTrue(allow?.isEnabled == true); allow?.performClick(nil)
        }
        client.onEvent = { value in
            event?(value)
            guard value["type"] as? String == "approval.requested", let original, let replacement = workspace.podSnapshot.approval else { return }
            XCTAssertEqual(original.identity, replacement.identity); XCTAssertNotEqual(original, replacement)
            XCTAssertFalse(allow?.isEnabled == true)
            XCTAssertFalse(workspace.resolveApproval(original, decision: "allow"))
            XCTAssertTrue(workspace.resolveApproval(replacement, decision: "deny"))
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
                XCTAssertTrue(workspace.podSnapshot.decisionPending); XCTAssertNil(workspace.podSnapshot.approvalError)
                XCTAssertFalse(workspace.resolveApproval(replacement, decision: "allow"))
                client.request("history.read", params: ["run": "review"]) { result in
                    if case .success(let data) = result { XCTAssertEqual((data["run"] as? [String: Any])?["text"] as? String, "2") }
                    else { XCTFail("Could not inspect decisions") }
                    checked.fulfill()
                }
            }
        }
        workspace.attach(); wait(for: [checked], timeout: 3)
    }

    func testLostApprovalReplyReportsUnknownOutcomeAndDoesNotAllowResubmission() throws {
        _ = NSApplication.shared
        let client = try peer(hello + """
        if (request.method === 'snapshot') reply(request,{workspace:'ws',session:'session',runtime:'runtime',seq:0,runs:[{id:'review',text:'Review',state:'awaiting_approval',session:'session',createdAt:1,revision:1,approval:{operation:'one',tool:'fixture',scope:'/workspace',args:'Arguments'}}]});
        if (request.method === 'files.list') reply(request,{entries:[]});
        """, timeout: 0.15)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        defer { client.onEvent = nil; client.onConnection = nil; client.forceStop(); workspace.window?.close() }
        let checked = expectation(description: "lost acknowledgement is not a rejection")
        let connection = client.onConnection
        var request: ApprovalRequest?, checking = false
        client.onConnection = { value in
            connection?(value)
            if client.isReady, request == nil {
                request = workspace.podSnapshot.approval
                XCTAssertTrue(workspace.resolveApproval(request!, decision: "allow"))
            } else if !client.isReady, let request, !checking {
                checking = true
                DispatchQueue.main.async {
                    XCTAssertTrue(workspace.podSnapshot.decisionPending)
                    XCTAssertTrue(workspace.podSnapshot.approvalError?.contains("outcome unknown") == true)
                    XCTAssertFalse(workspace.podSnapshot.canDecide)
                    XCTAssertFalse(workspace.resolveApproval(request, decision: "allow"))
                    checked.fulfill()
                }
            }
        }
        workspace.attach(); wait(for: [checked], timeout: 3)
    }

    func testExplicitStopRejectionRestoresRetryAndASecondStopCanComplete() throws {
        _ = NSApplication.shared
        let client = try peer("""
        if (request.method === 'hello') reply(request,{protocol:1,workspace:'ws',session:'session',runtime:'runtime',capabilities:['workspace-stop']});
        globalThis.attempts ??= 0;
        if (request.method === 'snapshot') reply(request,{workspace:'ws',session:'session',runtime:'runtime',seq:0,runs:[{id:'run',text:'Work',state:globalThis.attempts > 1 ? 'cancelled' : 'running',session:'session',createdAt:1,revision:globalThis.attempts+1}]});
        if (request.method === 'files.list') reply(request,{entries:[]});
        if (request.method === 'task.cancelAll') {
          globalThis.attempts++;
          if (globalThis.attempts === 1) send({kind:'response',id:request.id,error:{message:'Try Stop again'}});
          else reply(request,{requested:true});
        }
        """, timeout: 2)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        defer { client.onEvent = nil; client.onConnection = nil; client.forceStop(); workspace.window?.close() }
        let checked = expectation(description: "an explicit rejection releases the stop control")
        let connection = client.onConnection, event = client.onEvent
        var requested = false
        client.onConnection = { value in
            connection?(value)
            guard client.isReady, !requested else { return }; requested = true
            workspace.stop(); XCTAssertFalse(workspace.podSnapshot.canStop)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                XCTAssertTrue(workspace.podSnapshot.canStop)
                XCTAssertEqual(workspace.probeLayout["forceStopAvailable"] as? Bool, false)
                workspace.stop()
            }
        }
        client.onEvent = { value in
            event?(value)
            if requested, !workspace.hasWork { XCTAssertTrue(client.isReady); checked.fulfill() }
        }
        workspace.attach(); wait(for: [checked], timeout: 3)
    }

    func testClosingWorkspaceWaitsForShutdownReceiptWithoutRefreshingStoppedWorker() throws {
        _ = NSApplication.shared
        let client = try peer("""
        if (request.method === 'hello') reply(request,{protocol:1,workspace:'ws',session:'session',runtime:'runtime',capabilities:['workspace-stop']});
        if (request.method === 'snapshot') {
          if (globalThis.stopping) send({kind:'response',id:request.id,error:{message:'SHUTTING_DOWN'}});
          else reply(request,{workspace:'ws',session:'session',runtime:'runtime',seq:0,runs:[]});
        }
        if (request.method === 'files.list') reply(request,{entries:[]});
        if (request.method === 'task.cancelAll') reply(request,{requested:true});
        if (request.method === 'shutdown') {
          globalThis.stopping = true;
          setTimeout(() => {
            require('node:fs').writeFileSync('shutdown-receipt', 'durable');
            reply(request,{stopped:true,broccoliRetention:{chargedBytes:0,durable:true}});
          }, 100);
        }
        """, timeout: 2)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        defer { client.onEvent = nil; client.onConnection = nil; client.forceStop(); workspace.window?.close() }
        let closed = expectation(description: "workspace waits for durable shutdown")
        workspace.onClosed = { closed.fulfill() }
        let connection = client.onConnection
        var requested = false
        client.onConnection = { value in
            connection?(value)
            guard client.isReady, !requested else { return }; requested = true
            workspace.stopAndClose()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.03) {
                workspace.windowDidBecomeKey(Notification(name: NSWindow.didBecomeKeyNotification))
            }
        }
        workspace.attach(); wait(for: [closed], timeout: 3)
        XCTAssertTrue(FileManager.default.fileExists(atPath: directory.appendingPathComponent("shutdown-receipt").path))
        XCTAssertEqual(client.pendingRequestCount, 0)
    }

    func testWorkspaceStopIncludesUnseenQueuedWorkAndWaitsForAcknowledgment() throws {
        _ = NSApplication.shared
        let behavior = """
        if (request.method === 'hello') { reply(request, { protocol: 1, workspace: 'ws', session: 'session', runtime: 'runtime', capabilities: ['workspace-stop'] }); continue; }
        globalThis.runs ??= ['running', 'queued'].map((state, i) => ({ id: 'run-'+i, text: 'fixture '+i, state, session: 'session', createdAt: 1, activities: [], revision: 1 }));
        if (request.method === 'snapshot') {
          reply(request, { workspace: 'ws', session: 'session', runtime: 'runtime', seq: globalThis.stopped ? 1 : 0, runs: globalThis.stopped ? globalThis.runs : [globalThis.runs[0]] });
        } else if (request.method === 'task.cancelAll') {
          for (const run of globalThis.runs) { run.state = 'cancelled'; run.revision++; }
          globalThis.stopped = true;
          setTimeout(() => reply(request, { requested: true }), 100);
        } else if (request.method === 'files.list') { reply(request, { entries: [] }); }
        """
        let client = try peer(behavior, timeout: 1)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        defer { client.onEvent = nil; client.onConnection = nil; workspace.window?.close() }
        let checked = expectation(description: "atomic stop includes work missing from the client's last snapshot")
        let connection = client.onConnection, event = client.onEvent
        var requested = false
        client.onConnection = { value in
            connection?(value)
            if client.isReady && !requested { requested = true; workspace.stop(); XCTAssertTrue(workspace.hasWork) }
        }
        client.onEvent = { value in
            event?(value)
            guard requested, value["kind"] as? String == "snapshot", let payload = value["payload"] as? [String: Any], payload["seq"] as? Int == 1 else { return }
            XCTAssertEqual((payload["runs"] as? [[String: Any]])?.count, 2)
            XCTAssertFalse(workspace.hasWork); XCTAssertTrue(client.isReady); checked.fulfill()
        }
        workspace.attach(); wait(for: [checked], timeout: 3)
    }

    func testTurnScrubberLiveStageHeaderAndInlineWalkthroughNavigation() throws {
        _ = NSApplication.shared
        let behavior = """
        if (request.method === 'hello') { reply(request, { protocol: 1, workspace: 'ws', session: 'session', runtime: 'runtime', capabilities: ['walkthrough-v1'] }); continue; }
        globalThis.runs ??= [
          { id: 'failed-turn', text: 'Check normal prompt without failure keyword', state: 'failed', session: 'session', createdAt: 1, revision: 1, activities: [] },
          { id: 'active-turn', text: 'Implement telemetry', state: 'running', session: 'session', createdAt: 2, startedAt: Date.now() - 4000, revision: 2, activities: [
            { activityId: 'act-1', sequence: 1, startedSequence: 1, phase: 'tool', status: 'in_progress', message: 'Run swift test', detail: 'Executing package tests', timestamp: Date.now(), metadata: { scope: 'activity', itemType: 'run_command', attempt: 2, outputIdleMs: 12000, files: ['Sources/App.swift'] } },
            { activityId: 'turn-1', sequence: 2, startedSequence: 1, phase: 'thinking', status: 'in_progress', message: 'Thinking', detail: 'No stream update for 20s', timestamp: Date.now(), metadata: { scope: 'turn', completedSteps: 1, totalSteps: 3 } }
          ] }
        ];
        if (request.method === 'snapshot') { reply(request, { workspace: 'ws', session: 'session', runtime: 'runtime', seq: 1, runs: globalThis.runs }); continue; }
        if (request.method === 'activity.list') { reply(request, { run: request.params.run, activities: globalThis.runs[1].activities, activitySequence: 2, activityCount: 2 }); continue; }
        if (request.method === 'files.list') { reply(request, { entries: [] }); continue; }
        """
        let client = try peer(behavior, timeout: 1)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        defer { client.onEvent = nil; client.onConnection = nil; workspace.window?.close() }
        let checked = expectation(description: "live stage header, turn scrubber, and inline walkthrough navigation verified")
        let connection = client.onConnection
        var verified = false
        client.onConnection = { value in
            connection?(value)
            guard client.isReady, !verified else { return }; verified = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
                func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
                let views = descendants(workspace.window!.contentView!)
                let rail = views.compactMap { $0 as? TurnScrubberRail }.first!
                XCTAssertEqual(rail.turns.count, 2)
                XCTAssertTrue(rail.turns[0].isError, "Failed run must mark turn as error even when user text does not contain 'error'")
                XCTAssertFalse(rail.turns[0].isActive)
                XCTAssertTrue(rail.turns[1].isActive, "Active run must stay marked active even when activity rows follow the user row")
                XCTAssertFalse(rail.turns[1].isError)

                let labels = views.compactMap { $0 as? NSTextField }.map(\.stringValue)
                XCTAssertTrue(labels.contains { $0.contains("Running · Running a command") && $0.contains("Current batch · 1 of 3 actions finished") && $0.contains("No stream update for 20s") }, "Header chatState should surface live stage, batch progress, and turn wait detail: \(labels)")

                workspace.navigateProbe("expanded")
                let table = NSTableView()
                let cells = (0..<workspace.numberOfRows(in: table)).compactMap { workspace.tableView(table, viewFor: nil, row: $0) as? TranscriptMessageCell }
                let summaryCell = cells.first { $0.row.id == "active-turn-activity" }
                XCTAssertNotNil(summaryCell)
                XCTAssertTrue(summaryCell?.row.evidence?.contains("Current batch · 1 of 3 actions finished") == true)
                XCTAssertTrue(summaryCell?.row.evidence?.contains("No stream update for 20s") == true)
                XCTAssertTrue(summaryCell?.row.evidence?.contains("Attempt 2") == true)
                XCTAssertTrue(summaryCell?.row.evidence?.contains("Quiet 12s") == true)
                XCTAssertEqual(summaryCell?.row.actionTitle, "Open in Walkthrough")

                let childCell = cells.first { $0.row.id == "active-turn-act-1" }
                XCTAssertNotNil(childCell)
                XCTAssertTrue(childCell?.row.title.contains("Attempt 2") == true)
                XCTAssertTrue(childCell?.row.title.contains("Quiet 12s") == true)
                checked.fulfill()
            }
        }
        workspace.attach(); wait(for: [checked], timeout: 3)
    }
}

