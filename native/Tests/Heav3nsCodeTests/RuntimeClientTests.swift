// [LAYER: INFRASTRUCTURE]
import XCTest
import AppKit
@testable import Heav3nsCode

/// Real private pipes and a deliberately small protocol peer. Agent integration
/// is verified separately against the packaged LUMI worker.
final class RuntimeClientTests: RuntimeClientTestCase {
    func testAgentDeferStatusAcceptsHierarchyBarrierState() throws {
        let client = try peer(hello + snapshot + "if (request.method === 'task.deferStatus') { reply(request, { enabled: true, deferred: true, queued: 2, activeRun: 'run-1', boundary: 'safe_boundary', hierarchy: { deferred: true, activeChildren: 0, boundary: 'safe_boundary' } }); continue; }")
        let finished = expectation(description: "validated hierarchy defer status")
        client.onConnection = { _ in
            guard client.isReady else { return }
            client.request("task.deferStatus") { result in
                guard case .success(let value) = result else { XCTFail("Expected valid defer status: \(result)"); finished.fulfill(); return }
                XCTAssertEqual(value["deferred"] as? Bool, true)
                XCTAssertEqual(value["queued"] as? Int, 2)
                finished.fulfill()
            }
        }
        client.start(folder: directory)
        wait(for: [finished], timeout: 3)
    }

    func testSleepAssertionFollowsConfirmedRunStateAndWorkerExit() throws {
        let node = ["/opt/homebrew/bin/node", "/usr/local/bin/node", "/usr/bin/node"].first { FileManager.default.isExecutableFile(atPath: $0) }
        guard let node else { throw XCTSkip("Node is needed for the protocol peer") }
        var acquired = 0, released = 0
        let assertion = RunSleepAssertion(acquire: { acquired += 1; return 1 }, release: { _ in released += 1 })
        let client = try peer(hello + snapshot + """
        if (request.method === 'start') {
          send({kind:'event',workspace:'ws',session:'session',runtime:'runtime',type:'run.state',seq:1,payload:{id:'a',text:'work',state:'running',session:'session',createdAt:1}});
          reply(request,{}); continue;
        }
        if (request.method === 'finish') {
          send({kind:'event',workspace:'ws',session:'session',runtime:'runtime',type:'run.state',seq:2,payload:{id:'a',text:'work',state:'completed',session:'session',createdAt:1}});
          reply(request,{}); continue;
        }
        if (request.method === 'restart') {
          send({kind:'event',workspace:'ws',session:'session',runtime:'runtime',type:'run.state',seq:3,payload:{id:'b',text:'work',state:'running',session:'session',createdAt:1}});
          reply(request,{}); continue;
        }
        """, timeout: 2, nodeOverride: URL(fileURLWithPath: node), sleepAssertion: assertion)
        let exited = expectation(description: "worker exit releases assertion")
        client.onExit = { XCTAssertEqual(released, 2); exited.fulfill() }
        client.onConnection = { _ in
            guard client.isReady else { return }
            client.request("start") { _ in
                XCTAssertEqual(acquired, 1); XCTAssertEqual(released, 0)
                client.request("finish") { _ in
                    XCTAssertEqual(released, 1)
                    client.request("restart") { _ in
                        XCTAssertEqual(acquired, 2)
                        client.closeInput()
                        XCTAssertEqual(released, 1, "transport loss alone is not an idle boundary")
                    }
                }
            }
        }
        client.start(folder: directory)
        wait(for: [exited], timeout: 5)
    }

    func testRepeatedBurstsDrainOneHundredThousandEventsWithoutRetainingQueues() throws {
        let client = try peer(hello + snapshot + """
        if (request.method === 'burst') {
          const frames=Array.from({length:5000},(_,n)=>({v:1,kind:'event',workspace:'ws',session:'session',runtime:'runtime',type:'fixture',payload:{n}}));
          process.stdout.write(frames.map(JSON.stringify).join('\\n')+'\\n'); reply(request,{});
        }
        """, timeout: 5)
        func peakResidentBytes() -> Int64 {
            var usage = rusage(); _ = getrusage(RUSAGE_SELF, &usage)
            return Int64(usage.ru_maxrss)
        }
        let finished = expectation(description: "twenty bursts fully drained")
        var received = 0, rounds = 0, midPeak: Int64 = 0
        let initialPeak = peakResidentBytes()
        client.onEvent = { event in
            guard event["type"] as? String == "fixture" else { return }
            if (event["payload"] as? [String: Any])?["n"] as? Int != received % 5000 { XCTFail("Burst order changed") }
            received += 1
        }
        func next() {
            client.request("burst") { result in
                guard case .success = result else { XCTFail("Burst disconnected"); finished.fulfill(); return }
                rounds += 1
                XCTAssertEqual(received, rounds * 5000)
                XCTAssertEqual(client.pendingDeliveryBytes, 0); XCTAssertEqual(client.pendingRequestCount, 0)
                if rounds == 10 { midPeak = peakResidentBytes() }
                if rounds == 20 {
                    print("Transport burst memory: events=\(received), peakRSSInitial=\(initialPeak), peakRSSAfter50000=\(midPeak), peakRSSAfter100000=\(peakResidentBytes()) bytes (macOS process high-water RSS; no allocator baseline assertion)")
                    finished.fulfill()
                } else { DispatchQueue.main.async { next() } }
            }
        }
        client.onConnection = { _ in if client.isReady { next() } }
        client.start(folder: directory); wait(for: [finished], timeout: 30)
        client.onConnection = nil; client.onEvent = nil
        client.closeInput()
    }

    func testActiveRunFloodCannotGrowTheWorkspacePastItsAdvertisedQueue() throws {
        let client = try peer(hello + snapshot + """
        if (request.method === 'burst') {
          for(let n=1;n<=100;n++) send({kind:'event',workspace:'ws',session:'session',runtime:'runtime',type:'run.accepted',seq:n,
            payload:{id:'run-'+n,text:'queued',state:'queued',session:'session',createdAt:1}});
        }
        """)
        let ended = expectation(description: "invalid active cardinality disconnects")
        var received = 0
        client.onEvent = { if $0["type"] as? String == "run.accepted" { received += 1 } }
        client.onConnection = { _ in if client.isReady { client.request("burst") } }
        client.onExit = { ended.fulfill() }
        client.start(folder: directory); wait(for: [ended], timeout: 3)
        XCTAssertEqual(received, client.queueLimit + 1); XCTAssertFalse(client.isReady)
    }

    func testUnsafeEventClockAndTimestampAreRejectedBeforePresentation() throws {
        for (clock, timestamp) in [("9223372036854775807", "1"), ("1", "-1e300")] {
            let client = try peer(hello + snapshot + """
            if(request.method==='burst') process.stdout.write('{"v":1,"kind":"event","workspace":"ws","session":"session","runtime":"runtime","type":"run.state","seq":\(clock),"payload":{"id":"run","text":"task","state":"running","session":"session","createdAt":1,"startedAt":\(timestamp)}}\\n');
            """)
            let ended = expectation(description: "unsafe numeric state rejected")
            client.onEvent = { if $0["type"] as? String == "run.state" { XCTFail("Unsafe numbers reached the view") } }
            client.onConnection = { _ in if client.isReady { client.request("burst") } }
            client.onExit = { ended.fulfill() }
            client.start(folder: directory); wait(for: [ended], timeout: 3)
        }
    }

    func testClosedClientAndPendingCompletionReleaseTheirOwners() throws {
        final class Capture {}
        var client: RuntimeClient? = try peer(hello + snapshot, timeout: 30)
        weak var releasedClient: RuntimeClient?
        releasedClient = client
        var capture: Capture? = Capture()
        weak var releasedCapture: Capture?
        releasedCapture = capture
        let ended = expectation(description: "closed client exits")
        client?.onConnection = { [weak client] _ in
            guard let client, client.isReady else { return }
            client.request("files.search") { [retained = capture!] _ in withExtendedLifetime(retained) {} }
            client.closeInput()
        }
        client?.onExit = { ended.fulfill() }
        client?.start(folder: directory); wait(for: [ended], timeout: 3)
        clients.removeAll { $0 === client }; client = nil; capture = nil
        autoreleasepool { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
        XCTAssertNil(releasedClient); XCTAssertNil(releasedCapture)
    }

    func testProtocolFailureStopsLaterMessagesFromTheSameBurst() throws {
        let client = try peer(hello + snapshot + """
        if (request.method === 'burst') {
          const frames = [{v:1,kind:'protocol.error'}, {v:1,kind:'event',workspace:'ws',session:'session',runtime:'runtime',type:'fixture',payload:{}}];
          process.stdout.write(frames.map(JSON.stringify).join('\\n') + '\\n');
        }
        """)
        let ended = expectation(description: "protocol failure stops worker")
        client.onEvent = { if $0["type"] as? String == "fixture" { XCTFail("Post-failure event escaped") } }
        client.onConnection = { _ in if client.isReady { client.request("burst") } }
        client.onExit = { ended.fulfill() }
        client.start(folder: directory); wait(for: [ended], timeout: 3)
        XCTAssertEqual(client.pendingDeliveryBytes, 0); XCTAssertEqual(client.pendingRequestCount, 0)
    }

    func testResponseCallbackCanCloseTheChannelBeforeLaterBurstEvents() throws {
        let client = try peer(hello + snapshot + """
        if (request.method === 'burst') {
          process.stdout.write([
            {v:1,kind:'response',id:request.id,result:{}},
            {v:1,kind:'event',workspace:'ws',session:'session',runtime:'runtime',type:'fixture',payload:{}}
          ].map(JSON.stringify).join('\\n') + '\\n');
        }
        """)
        let ended = expectation(description: "callback closes channel")
        client.onEvent = { if $0["type"] as? String == "fixture" { XCTFail("Callback close was ignored") } }
        client.onConnection = { _ in if client.isReady { client.request("burst") { _ in client.closeInput() } } }
        client.onExit = { ended.fulfill() }
        client.start(folder: directory); wait(for: [ended], timeout: 3)
    }

    func testBurstYieldsToTheMainQueueAndDrainsBeforeProcessExit() throws {
        let client = try peer(hello + snapshot + """
        if (request.method === 'burst') {
          const frames = Array.from({length:1000},(_,n)=>({v:1,kind:'event',workspace:'ws',session:'session',runtime:'runtime',type:'fixture',payload:{n}}));
          process.stdout.write(frames.map(JSON.stringify).join('\\n') + '\\n', () => process.exit(0));
        }
        """, timeout: 3)
        let ended = expectation(description: "all output drained before exit")
        var received = 0, atYield = 0
        client.onEvent = { event in
            guard event["type"] as? String == "fixture" else { return }
            XCTAssertEqual((event["payload"] as? [String: Any])?["n"] as? Int, received)
            received += 1
            if received == 1 { DispatchQueue.main.async { atYield = received } }
        }
        client.onConnection = { _ in if client.isReady { client.request("burst") } }
        client.onExit = { ended.fulfill() }
        client.start(folder: directory); wait(for: [ended], timeout: 5)
        XCTAssertEqual(received, 1000); XCTAssertGreaterThan(atYield, 0)
        XCTAssertLessThanOrEqual(atYield, ControlFrameInbox.batchLimit)
        XCTAssertEqual(client.pendingDeliveryBytes, 0)
    }

    func testSnapshotWaitersAreBoundedAndReleasedWhenRefreshOverflows() throws {
        let client = try peer(hello + """
        if (request.method === 'snapshot' && !globalThis.attached) {
          globalThis.attached = true;
          reply(request,{workspace:'ws',session:'session',runtime:'runtime',seq:0,runs:[]});
        }
        """, timeout: 3)
        let ended = expectation(description: "bounded refresh fanout")
        client.onConnection = { _ in
            guard client.isReady else { return }
            for _ in 0..<RuntimeClient.snapshotWaiterLimit { client.requestSnapshot { XCTFail("Unconfirmed snapshot") } }
            XCTAssertEqual(client.pendingSnapshotCount, RuntimeClient.snapshotWaiterLimit)
            client.requestSnapshot { XCTFail("Overflow snapshot") }
            XCTAssertFalse(client.isReady); XCTAssertEqual(client.pendingSnapshotCount, 0)
        }
        client.onExit = { ended.fulfill() }
        client.start(folder: directory); wait(for: [ended], timeout: 5)
    }

    func testClosingInputImmediatelyRevokesReadinessAndCompletesPendingRequestsOnce() throws {
        let client = try peer(hello + snapshot, timeout: 2)
        let finished = expectation(description: "connection close completes its observer and pending request")
        var started = false, disconnects = 0, completions = 0
        client.onConnection = { message in
            if message.hasPrefix("Disconnected") { disconnects += 1; XCTAssertFalse(client.isReady) }
            guard client.isReady, !started else { return }; started = true
            client.request("models.list") { result in
                completions += 1
                if case .success = result { XCTFail("Closed requests must fail") }
            }
            client.closeInput(); client.closeInput()
            XCTAssertFalse(client.isReady)
            XCTAssertEqual(disconnects, 1); XCTAssertEqual(completions, 1)
            finished.fulfill()
        }
        client.start(folder: directory); wait(for: [finished], timeout: 3)
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        XCTAssertEqual(disconnects, 1); XCTAssertEqual(completions, 1)
    }

    func testFreshSnapshotCannotJoinAReadFromBeforeAConversationMutation() throws {
        let client = try peer(hello + """
        if (request.method === 'snapshot') {
          globalThis.snapshots = (globalThis.snapshots || 0) + 1;
          const count = globalThis.snapshots;
          const value = {workspace:'ws',session:'session',runtime:'runtime',seq:0,runs:[],conversation:count > 2 ? 'new' : 'old'};
          if (count === 2) setTimeout(() => reply(request,value),80); else reply(request,value);
        }
        """, timeout: 2)
        var conversations = [String](), started = false
        let finished = expectation(description: "fresh snapshot follows the mutation")
        client.onEvent = { if let payload = $0["payload"] as? [String: Any], let id = payload["conversation"] as? String { conversations.append(id) } }
        client.onConnection = { _ in
            guard client.isReady, !started else { return }; started = true
            client.requestSnapshot()
            client.requestSnapshot(afterPending: true) { finished.fulfill() }
        }
        client.start(folder: directory); wait(for: [finished], timeout: 3)
        XCTAssertEqual(conversations, ["old", "new"])
    }

    func testConversationSwitchClearsInspectionHistoryAndKeepsDraftsDespiteLateReplies() throws {
        _ = NSApplication.shared
        let client = try peer("""
        globalThis.selected ??= 'first';
        if (request.method === 'hello') reply(request,{protocol:1,workspace:'ws',session:'session',runtime:'runtime',capabilities:['conversations','conversation-context-v1']});
        if (request.method === 'snapshot') reply(request,{workspace:'ws',session:'session',runtime:'runtime',seq:0,runs:[],conversation:globalThis.selected});
        if (request.method === 'conversation.list') reply(request,{conversations:[{id:'second',title:'Second'},{id:'first',title:'First'}],more:false});
        if (request.method === 'conversation.create') { globalThis.selected='second'; setTimeout(() => reply(request,{conversation:'second'}),20); }
        if (request.method === 'conversation.select') { globalThis.selected=request.params.id; reply(request,{conversation:globalThis.selected}); }
        if (request.method === 'files.list') reply(request,{entries:[]});
        if (request.method === 'files.read') setTimeout(() => reply(request,{text:'Delayed old preview',revision:'r',nextOffset:19,size:19}),200);
        if (request.method === 'context.inspect') setTimeout(() => reply(request,{conversation:'first',activeMessages:79,rollingSummaries:1,savedMemories:3,working:false,queued:0,observedAt:Date.now()}),210);
        """, timeout: 2)
        let drafts = WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts"))
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: drafts)
        func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
        let composer = descendants(workspace.window!.contentView!).compactMap { $0 as? ComposerView }.first!
        let connected = client.onConnection, events = client.onEvent
        let finished = expectation(description: "late inspection cannot revive the previous thread")
        var started = false, checked = false
        client.onConnection = { value in
            connected?(value)
            guard client.isReady, !started else { return }; started = true
            composer.string = "Keep my first draft 🌙"; workspace.prepareToClose()
            workspace.inspect(.file("slow.txt")); workspace.inspect(.context("first")); workspace.newConversation()
            XCTAssertFalse(composer.isEditable)
        }
        client.onEvent = { value in
            events?(value)
            guard (value["payload"] as? [String: Any])?["conversation"] as? String == "second", !checked else { return }; checked = true
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                XCTAssertFalse(workspace.canInspectBack)
                XCTAssertFalse(workspace.window?.isVisible == true, "An asynchronous conversation reply must not present a background workspace")
                XCTAssertFalse((workspace.probeLayout["inspectionPreview"] as? String ?? "").contains("Delayed"))
                XCTAssertFalse((workspace.probeLayout["inspectionPreview"] as? String ?? "").contains("79 active"))
                XCTAssertEqual(workspace.probeLayout["inspectionTitle"] as? String, "Inspect")
                XCTAssertEqual(composer.string, ""); XCTAssertTrue(composer.isEditable)
                XCTAssertEqual(try? drafts.load()?.conversations?["first"], "Keep my first draft 🌙")
                composer.string = "Keep second draft"; workspace.prepareToClose()
                workspace.navigateProbe("previous-conversation")
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) {
                    XCTAssertEqual(composer.string, "Keep my first draft 🌙")
                    XCTAssertEqual(try? drafts.load()?.conversations?["second"], "Keep second draft")
                    finished.fulfill()
                }
            }
        }
        workspace.attach(); wait(for: [finished], timeout: 4)
        client.onConnection = nil; client.onEvent = nil; client.forceStop(); workspace.window?.close()
    }

    func testFocusComposerDismissesTheModelessFilePickerAndRestoresKeyboardTarget() throws {
        _ = NSApplication.shared
        let client = try peer("""
        if (request.method === 'hello') reply(request,{protocol:1,workspace:'ws',session:'session',runtime:'runtime',capabilities:['file-search-v1']});
        if (request.method === 'snapshot') reply(request,{workspace:'ws',session:'session',runtime:'runtime',seq:0,runs:[]});
        if (request.method === 'files.list') reply(request,{entries:[]});
        if (request.method === 'files.search') reply(request,{entries:[],scope:'git',matched:0,more:false,scanLimited:false});
        """, timeout: 2)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        let finished = expectation(description: "composer owns focus after the picker is dismissed")
        let connected = client.onConnection
        var started = false
        client.onConnection = { value in
            connected?(value)
            guard client.isReady, !started else { return }; started = true
            workspace.window?.makeKeyAndOrderFront(nil); workspace.showQuickOpen()
            XCTAssertEqual((workspace.probeLayout["quickOpen"] as? [String: Any])?["visible"] as? Bool, true)
            XCTAssertEqual(workspace.window?.childWindows?.count, 1)
            workspace.focusComposer()
            XCTAssertEqual((workspace.probeLayout["quickOpen"] as? [String: Any])?["visible"] as? Bool, false)
            XCTAssertTrue(workspace.window?.childWindows?.isEmpty ?? true)
            XCTAssertEqual(workspace.probeLayout["composerFocused"] as? Bool, true)
            XCTAssertTrue(workspace.window?.isVisible == true)
            finished.fulfill()
        }
        workspace.attach(); wait(for: [finished], timeout: 3)
        client.onConnection = nil; client.forceStop(); workspace.window?.close()
    }

    func testLostAcknowledgmentRestoresRequestIdentityAndPreservesANewerDraftWithoutReplay() throws {
        _ = NSApplication.shared
        let drafts = WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts"))
        let pathLiteral = String(data: try JSONEncoder().encode(drafts.file.path), encoding: .utf8)!
        let client = try peer(hello + snapshot + """
        if (request.method === 'files.list') reply(request, { entries: [] });
        if (request.method === 'task.submit') {
          const saved = JSON.parse(require('node:fs').readFileSync(\(pathLiteral), 'utf8'));
          send({ kind: 'event', workspace: 'ws', session: 'session', runtime: 'runtime', type: 'fixture.saved', seq: 1, payload: { persisted: saved.submission.id === request.id && saved.submission.text === request.params.text } });
        }
        """, timeout: 0.2)
        let first = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: drafts)
        func composer(_ workspace: WorkspaceWindow) -> ComposerView? {
            func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
            return workspace.window?.contentView.map(descendants)?.compactMap { $0 as? ComposerView }.first
        }
        let persisted = expectation(description: "request identity saved before dispatch")
        let disconnected = expectation(description: "lost acknowledgment disconnects")
        let connection = client.onConnection, event = client.onEvent
        var sent = false, expired = false
        client.onConnection = { value in
            connection?(value)
            if client.isReady, !sent { sent = true; first.submitProbe("original instruction") }
            else if sent, !client.isReady, !expired { expired = true; disconnected.fulfill() }
        }
        client.onEvent = { value in
            event?(value)
            if value["type"] as? String == "fixture.saved" {
                XCTAssertEqual((value["payload"] as? [String: Any])?["persisted"] as? Bool, true)
                composer(first)?.string = "A newer draft 🌱"
                XCTAssertTrue(first.prepareToClose()); persisted.fulfill()
            }
        }
        first.attach(); wait(for: [persisted, disconnected], timeout: 3)
        let submission = try XCTUnwrap(drafts.load()?.submission)
        XCTAssertEqual(submission.text, "original instruction")
        client.onConnection = nil; client.onEvent = nil; client.forceStop(); first.window?.close()

        let recovery = try peer(hello + snapshot + """
        globalThis.submits ??= 0;
        if (request.method === 'files.list') reply(request, { entries: [] });
        if (request.method === 'task.submit') globalThis.submits++;
        if (request.method === 'task.lookup') {
          reply(request, { found: true, run: { id: 'recorded', request: request.params.request, text: 'original instruction', state: 'interrupted', session: 'session', createdAt: 1, revision: 1 } });
          send({ kind: 'event', workspace: 'ws', session: 'session', runtime: 'runtime', type: 'fixture.reconciled', seq: 1, payload: { request: request.params.request, submits: globalThis.submits } });
        }
        """, timeout: 1)
        let reopened = WorkspaceWindow(folder: directory, runtimeClient: recovery, draftStore: drafts)
        defer { recovery.onEvent = nil; recovery.onConnection = nil; reopened.window?.close() }
        XCTAssertEqual(composer(reopened)?.string, "A newer draft 🌱")
        let reconciled = expectation(description: "reopen looks up the original request without replaying it")
        let recoveryEvent = recovery.onEvent
        recovery.onEvent = { value in
            recoveryEvent?(value)
            if value["type"] as? String == "fixture.reconciled" {
                let payload = value["payload"] as? [String: Any]
                XCTAssertEqual(payload?["request"] as? String, submission.id)
                XCTAssertEqual(payload?["submits"] as? Int, 0)
                XCTAssertEqual(composer(reopened)?.string, "A newer draft 🌱")
                XCTAssertEqual(try? drafts.load()?.text, "A newer draft 🌱")
                XCTAssertNil(try? drafts.load()?.submission)
                reconciled.fulfill()
            }
        }
        reopened.attach(); wait(for: [reconciled], timeout: 3)
    }

    func testApprovalDecisionStaysPendingAcrossSnapshotsAndCannotApplyToTheNextRequest() throws {
        _ = NSApplication.shared
        let behavior = """
        if (request.method === 'hello') { reply(request, { protocol: 1, workspace: 'ws', session: 'session', runtime: 'runtime', capabilities: ['approval'] }); continue; }
        globalThis.run ??= { id: 'review', text: 'Review operations', state: 'awaiting_approval', session: 'session', createdAt: 1, revision: 1, approval: { operation: 'one', tool: 'fixture', scope: '/workspace', args: 'Original arguments', reason: 'Review first' } };
        globalThis.decisions ??= 0;
        if (request.method === 'snapshot') reply(request, { workspace: 'ws', session: 'session', runtime: 'runtime', seq: 0, runs: [globalThis.run] });
        if (request.method === 'files.list') reply(request, { entries: [] });
        if (request.method === 'approval.resolve') {
          globalThis.decisions++;
          setTimeout(() => {
            globalThis.run.revision++; globalThis.run.approval.operation = 'two'; globalThis.run.approval.args = 'New arguments';
            send({ kind: 'event', workspace: 'ws', session: 'session', runtime: 'runtime', run: 'review', type: 'approval.requested', seq: 1, payload: globalThis.run });
            reply(request, { resolved: true });
          }, 300);
        }
        if (request.method === 'history.read') reply(request, { run: { ...globalThis.run, text: String(globalThis.decisions) }, outputAvailable: false });
        """
        let client = try peer(behavior, timeout: 1)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        defer {
            if let parent = workspace.window, let sheet = parent.attachedSheet { parent.endSheet(sheet); sheet.orderOut(nil) }
            client.onEvent = nil; client.onConnection = nil; workspace.window?.close()
        }
        let checked = expectation(description: "pending and stale reviews cannot send another decision")
        let connection = client.onConnection
        var requested = false
        client.onConnection = { value in
            connection?(value)
            guard client.isReady, !requested else { return }; requested = true
            client.requestSnapshot {
                workspace.showApprovalReview()
                func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
                let views = workspace.window?.attachedSheet?.contentView.map(descendants) ?? []
                let allow = views.compactMap { $0 as? NSButton }.first { $0.title == "Allow Once" }
                XCTAssertNotNil(allow); allow?.performClick(nil)
                client.requestSnapshot {
                    XCTAssertEqual(allow?.isEnabled, false)
                    allow?.performClick(nil)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.45) {
                        XCTAssertEqual(allow?.isEnabled, false)
                        XCTAssertTrue(views.compactMap { $0 as? NSTextView }.first?.string.contains("Original arguments") == true)
                        XCTAssertTrue(views.compactMap { $0 as? NSTextField }.contains { $0.stringValue.contains("no longer waiting") })
                        allow?.performClick(nil)
                        client.request("history.read", params: ["run": "review"]) { result in
                            if case .success(let data) = result { XCTAssertEqual((data["run"] as? [String: Any])?["text"] as? String, "1") }
                            else { XCTFail("Could not inspect decision count") }
                            checked.fulfill()
                        }
                    }
                }
            }
        }
        client.start(folder: directory); wait(for: [checked], timeout: 4)
    }

    func testQueueEditorRespondsWhenItsInstructionStarts() throws {
        _ = NSApplication.shared
        let behavior = """
        if (request.method === 'hello') { reply(request, { protocol: 1, workspace: 'ws', session: 'session', runtime: 'runtime', capabilities: ['run-details', 'queue-edit'] }); continue; }
        globalThis.run ??= { id: 'queued', text: 'Original waiting instruction', state: 'queued', session: 'session', createdAt: 1, revision: 1 };
        if (request.method === 'snapshot') reply(request, { workspace: 'ws', session: 'session', runtime: 'runtime', seq: 0, runs: [globalThis.run] });
        if (request.method === 'files.list') reply(request, { entries: [] });
        if (request.method === 'history.read') {
          reply(request, { run: globalThis.run, outputAvailable: false });
          setTimeout(() => {
            globalThis.run.state = 'running'; globalThis.run.revision++;
            send({ kind: 'event', workspace: 'ws', session: 'session', runtime: 'runtime', run: 'queued', type: 'run.state', seq: 1, payload: globalThis.run });
          }, 100);
        }
        """
        let client = try peer(behavior, timeout: 1)
        let workspace = WorkspaceWindow(folder: directory, runtimeClient: client, draftStore: WorkspaceDraftStore(folder: directory, root: directory.appendingPathComponent("drafts")))
        defer {
            if let parent = workspace.window, let sheet = parent.attachedSheet { parent.endSheet(sheet); sheet.orderOut(nil) }
            client.onEvent = nil; client.onConnection = nil; workspace.window?.close()
        }
        let checked = expectation(description: "live queue state invalidates an open edit")
        let connection = client.onConnection, event = client.onEvent
        var requested = false
        client.onEvent = { value in
            event?(value)
            guard value["type"] as? String == "run.state", (value["payload"] as? [String: Any])?["state"] as? String == "running" else { return }
            func descendants(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(descendants) }
            let views = workspace.window?.attachedSheet?.contentView.map(descendants) ?? []
            let text = views.compactMap { $0 as? NSTextView }.first
            text?.string = "Preserved follow-up"; text?.didChangeText()
            XCTAssertEqual(views.compactMap { $0 as? NSButton }.first { $0.title == "Save Changes" }?.isEnabled, false)
            XCTAssertEqual(views.compactMap { $0 as? NSButton }.first { $0.title == "Copy Edit" }?.isEnabled, true)
            XCTAssertTrue(views.compactMap { $0 as? NSTextField }.contains { $0.stringValue.contains("no longer waiting") })
            XCTAssertEqual(text?.string, "Preserved follow-up")
            checked.fulfill()
        }
        client.onConnection = { value in
            connection?(value)
            guard client.isReady, !requested else { return }; requested = true
            client.requestSnapshot { workspace.navigateProbe("queue-edit") }
        }
        workspace.attach(); wait(for: [checked], timeout: 4)
    }
}
