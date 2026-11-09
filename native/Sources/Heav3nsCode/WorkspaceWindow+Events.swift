// [LAYER: UI]
import AppKit
import Foundation

extension WorkspaceWindow {
    func configureClient() {
        viewportBranchGeneration += 1; viewportBranchCheckPending = false; viewportBranchCheckFailureShown = false
        let source = client
        projectTaskRevision = -1; projectTaskDisplayedRevision = -1; projectTaskOffset = 0; projectTaskPreviousOffsets = []; projectTaskMovePending = false; projectTasksLoading = false; projectTasksRefreshPending = false
        client.onNotice = { [weak self, weak source] message in
            guard let self, self.client === source else { return }
            self.showNotice(message)
        }
        client.onConnection = { [weak self, weak source] value in
            guard let self, let source, self.client === source else { return }; self.connection = value; self.updateStatus(); self.walkthroughLoaded = nil; self.updateWalkthrough()
            self.modelPicker?.connectionChanged()
            self.projectRun.connect(self.client)
            if self.client.isReady {
                source.request("agentSwarm.settings", params: ["action": "get"]) { [weak self, weak source] result in
                    guard let self, let source, self.client === source, case .success(let settings) = result else { return }
                    let master = settings["enabled"] as? Bool == true
                    self.criticalReviewEnabled = master || settings["criticalCompletionReview"] as? Bool == true
                    self.legionEnabled = master || settings["legion"] as? Bool == true
                    self.maxSpecialists = settings["maxSpecialists"] as? Int ?? 4
                    self.updateStatus()
                }
                source.request("task.deferStatus") { [weak self, weak source] result in
                    guard let self, let source, self.client === source, case .success(let status) = result else { return }
                    self.swarmDeferEnabled = status["enabled"] as? Bool == true
                    self.swarmDeferred = status["deferred"] as? Bool == true
                    self.updateStatus()
                }
                self.reconnectButton.isHidden = true
                if self.recovering && self.uncertainSubmission == nil { self.showNotice("Reconnected. Recorded history is restored; no work was restarted.") }
                self.recovering = false
                self.reconcileSubmission()
                self.refreshNavigation()
                self.refreshProjectTasks()
                self.loadConversations()
                if self.client.capabilities.contains("model-selection-v1") {
                    source.request("models.list", params: [:]) { [weak self, weak source] result in
                        guard let self, self.client === source else { return }
                        if case .success(let data) = result, let selection = WorkspaceModelSelection(data) { self.updateModel(selection) }
                    }
                }
                self.probe?.connected(self)
            } else if value != "Connecting to LUMI…" {
                self.walkthrough.projectTasksUnavailable("Connect to LUMI to view project tasks.")
                self.walkthrough.setProjectTaskMovesEnabled(false)
                self.reasoningPicker.connectionLost()
                self.switchingConversation = false; self.expectedConversation = nil
                self.quickOpen?.dismiss()
                self.stopThenSend = nil
                self.showNotice(value)
                self.reconnectButton.isHidden = self.client.process?.isRunning == true
                self.updateStatus()
            }
        }
        client.onEvent = { [weak self, weak source] event in
            guard let self, self.client === source else { return }; self.receive(event)
        }
        client.onExit = { [weak self, weak source] in
            guard let self, self.client === source else { return }
            self.stopped = true; self.reconnectButton.isHidden = self.closing
            self.elapsedTimer?.invalidate(); self.elapsedTimer = nil
            if self.closing { self.window?.close() }
            else { self.updateStatus() }
        }
    }

    func configurePods(_ show: @escaping () -> Void) { conversations.onPods = { [weak self] in guard let self, self.saveDraft() else { return }; show() } }

    var podSnapshot: PodSnapshot {
        let run = runs.first { $0["id"] as? String == active } ?? runs.last
        let queued = runs.filter { $0["state"] as? String == "queued" }.count
        let review = currentApproval != nil
        let runState = run?["state"] as? String ?? ""
        let activities = run?["activities"] as? [[String: Any]] ?? []
        let disconnected = connection == "Preparing" || connection == "Connecting to LUMI…" ? "Connecting" : "Disconnected"
        let terminalLabel = ["failed": "Failed", "interrupted": "Interrupted", "completed": "Completed", "cancelled": "Stopped", "timed_out": "Timed out"][runState] ?? "Idle"
        let label = !client.isReady ? disconnected : switchingConversation ? "Opening thread" : cancelRequested ? "Stopping" : uncertainSubmission != nil ? "Check connection" : decidingApproval != nil ? "Saving decision" : review ? "Needs review" : pendingRequest != nil ? "Submitting" : hasWork ? "Working" : terminalLabel
        let parsedActivities = run.map(WalkthroughProjection.activities) ?? []
        let activeActivity = parsedActivities.filter { $0.isActive && $0.scope != "turn" }.max { $0.sequence < $1.sequence } ?? parsedActivities.max { $0.sequence < $1.sequence }
        let podActivity = !client.isReady ? "Open thread to reconnect" : pendingRequest != nil ? "Submitting instruction…" : activeActivity.map { $0.scope == "turn" && !$0.detail.isEmpty ? $0.detail : $0.displayTitle } ?? (run?["activities"] as? [[String: Any]])?.last?["message"] as? String ?? (hasWork ? "Agent is working" : "Ready for another instruction")
        return PodSnapshot(path: folder.path, thread: chatTitle.stringValue, status: label,
            activity: podActivity,
            instruction: String((run?["text"] as? String ?? "").prefix(220)), response: String((run?["response"] as? String ?? "").prefix(300)),
            queued: queued, running: client.isReady && hasWork && !review, needsReview: review,
            canStop: client.isReady && hasWork && !cancelRequested && !closing,
            stream: PodStreamProjection.text(run), streamIdentity: "\(currentConversation ?? ""):\(run?["id"] as? String ?? "")",
            approval: currentApproval, canDecide: client.isReady && decidingApproval == nil && !cancelRequested && !closing && !switchingConversation,
            decisionPending: decidingApproval != nil, approvalError: approvalFailure,
            connected: client.isReady, startedAt: run?["startedAt"] as? Double ?? (hasWork ? run?["createdAt"] as? Double : nil),
            finishedAt: terminalStates.contains(runState) ? run?["updatedAt"] as? Double : nil,
            lastActivityAt: activities.compactMap { $0["timestamp"] as? Double }.max(),
            approvalRequestedAt: review ? (run?["approval"] as? [String: Any])?["createdAt"] as? Double : nil,
            activityCount: max(run?["activityCount"] as? Int ?? 0, activities.count),
            canReconnect: !client.isReady && client.process?.isRunning != true && !closing,
            canOpenCheckpoints: client.isReady && client.capabilities.contains("checkpoints-v1") && !switchingConversation && !closing && window?.attachedSheet == nil,
            contextID: podContextID, stopNeedsAttention: cancelRequested && forceAvailable)
    }

    func matchesPodTarget(_ target: PodTarget) -> Bool { target.path == folder.path && target.context == podContextID && !closing }

    func attach() { client.start(folder: folder); window?.makeFirstResponder(composer) }

    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [.init("navigation"), .init("history"), .init("jumpBar"), .flexibleSpace, .init("project"), .init("state"), .init("stop"), .init("actions")]
    }

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] { toolbarDefaultItemIdentifiers(toolbar) }

    func toolbar(_ toolbar: NSToolbar, itemForItemIdentifier identifier: NSToolbarItem.Identifier, willBeInsertedIntoToolbar flag: Bool) -> NSToolbarItem? {
        let item = NSToolbarItem(itemIdentifier: identifier)
        switch identifier.rawValue {
        case "history":
            item.label = "History"
            item.toolTip = "Navigate Back (⌘[) or Forward (⌘])"
            item.view = toolbarHistory
        case "jumpBar":
            item.label = "Jump Bar"
            item.toolTip = "Current Workspace Context (Jump Bar)"
            item.view = jumpBar
        case "project":
            item.label = "Run Project"; item.toolTip = "Run Project (⌘R)"
            item.image = NSImage(systemSymbolName: "play.circle", accessibilityDescription: "Run Project")
            item.target = self; item.action = #selector(showProjectRun)
        case "navigation":
            item.label = "Navigation"; item.toolTip = "Toggle Navigation (⌘B)"
            item.image = NSImage(systemSymbolName: "sidebar.left", accessibilityDescription: "Toggle Navigation")
            item.target = self; item.action = #selector(toggleNavigation)
        case "state":
            item.label = "Runtime State"; item.view = status
            status.widthAnchor.constraint(lessThanOrEqualToConstant: 260).isActive = true
        case "stop": item.label = "Stop LUMI"; item.view = stopButton; item.visibilityPriority = .user
        case "actions":
            item.label = "Actions"; item.toolTip = "Workspace Actions (⌘⇧P)"
            item.image = NSImage(systemSymbolName: "ellipsis.circle", accessibilityDescription: "Workspace Actions")
            item.target = self; item.action = #selector(showActions)
        default: return nil
        }
        return item
    }

    @objc func toolbarHistoryClicked() {
        if toolbarHistory.selectedSegment == 0 {
            inspectBack()
        } else if toolbarHistory.selectedSegment == 1 {
            inspectForward()
        }
    }

    @objc func openJumpBarSegment() {
        guard let item = jumpBar.clickedPathItem, let url = item.url else { return }
        var isDir: ObjCBool = false
        if FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir) {
            if isDir.boolValue {
                showFiles()
            } else {
                readFile(url.path)
            }
        }
    }

    func updateJumpBar() {
        switch inspectionHistory.current {
        case .file(let path):
            let fullURL = path.hasPrefix("/") ? URL(fileURLWithPath: path) : folder.appendingPathComponent(path)
            jumpBar.url = fullURL
        case .diff(let path, _):
            let fullURL = path.hasPrefix("/") ? URL(fileURLWithPath: path) : folder.appendingPathComponent(path)
            jumpBar.url = fullURL
        default:
            if let convo = currentConversation, !convo.isEmpty {
                let title = chatTitle.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
                jumpBar.url = folder.appendingPathComponent(title.isEmpty ? "Chat" : title)
            } else {
                jumpBar.url = folder
            }
        }
    }

    func updateModel(_ selection: WorkspaceModelSelection) {
        reasoningPicker.receive(selection)
        reasoningPicker.refresh(supported: client.isReady && client.capabilities.contains("reasoning-selection-v1"))
        modelButton.title = selection.model
        modelButton.toolTip = "\(selection.providerLabel) · \(selection.model) · Choose model and provider for this workspace"
        modelButton.setAccessibilityLabel("\(selection.providerLabel), model \(selection.model). Choose model and provider")
    }

    @objc func showModels() {
        guard canChooseModels, let window else { return }
        if let picker = modelPicker {
            if picker.window?.isVisible == true { picker.window?.makeKeyAndOrderFront(nil) }
            else { picker.present(for: window) }
            return
        }
        modelPicker = ModelPicker(request: { [weak self] method, params, completion in
            guard let self else { completion(.failure(BridgeError.message("Workspace closed."))); return }
            self.client.request(method, params: params, completion: completion)
        }, connected: { [weak self] in
            self?.client.isReady == true && self?.client.capabilities.contains("model-selection-v1") == true
        }, supportsDiscovery: { [weak self] in self?.client.capabilities.contains("model-discovery-v1") == true }, supportsCredentials: { [weak self] in self?.client.capabilities.contains("openrouter-credentials-v1") == true }, canApply: { [weak self] in
            guard let self else { return false }
            return self.canRequestRecovery && self.stopThenSend == nil && !self.reasoningPicker.pending
        }, onSelection: { [weak self] in self?.updateModel($0) })
        modelPicker?.present(for: window)
    }

    func buildActions() -> [WorkspaceAction] {
        var actions = [
            WorkspaceAction(title: "Open Workspace…", shortcut: "⌘O") { NSApp.sendAction(#selector(ApplicationDelegate.openFolder), to: NSApp.delegate, from: nil) },
            WorkspaceAction(title: "Focus Composer", shortcut: "⌘L") { [weak self] in self?.focusComposer() },
            WorkspaceAction(title: "Show Chats", shortcut: "⇧⌘C", keywords: ["chat", "conversations", "threads", "sidebar"]) { [weak self] in self?.showChats() },
            WorkspaceAction(title: "Show Files", shortcut: "⌘1") { [weak self] in self?.showFiles() },
            WorkspaceAction(title: "Show Changes", shortcut: "⌘2") { [weak self] in self?.showChanges() },
            WorkspaceAction(title: "Show Recorded Runs", shortcut: "⌘3", keywords: ["history", "sessions"]) { [weak self] in self?.showSessions() },
            WorkspaceAction(title: "Show Walkthrough", shortcut: "⌘6") { [weak self] in self?.showWalkthrough() },
            WorkspaceAction(title: "Show Up Next", shortcut: "⌘4", keywords: ["queue", "pending instructions"]) { [weak self] in self?.showQueue() },
            WorkspaceAction(title: "Toggle Navigation", shortcut: "⌘B", keywords: ["sidebar"]) { [weak self] in self?.toggleNavigation() },
            WorkspaceAction(title: "Go to Line…", shortcut: "⌘G", keywords: ["jump", "line", "offset"]) { [weak self] in self?.showGoToLine() },
            WorkspaceAction(title: "Go to Symbol in File…", shortcut: "⇧⌘O", keywords: ["symbol", "declaration", "method", "function"]) { [weak self] in self?.showGoToSymbol() },
            WorkspaceAction(title: "Command Palette…", shortcut: "⌘K", keywords: ["commands", "palette", "actions"]) { [weak self] in self?.showCommandPalette() },
            WorkspaceAction(title: "Refresh Current Destination", shortcut: "") { [weak self] in self?.refreshNavigation() }
        ]
        if reasoningPicker.isEnabled { actions.insert(WorkspaceAction(title: "Change Reasoning Level…", shortcut: "", keywords: ["effort", "thinking", "speed", "depth"]) { [weak self] in self?.reasoningPicker.performClick(nil) }, at: 1) }
        if client.capabilities.contains("model-selection-v1") { actions.insert(WorkspaceAction(title: "Choose Model & Provider…", shortcut: "⇧⌘M", keywords: ["switch", "change", "AI"]) { [weak self] in self?.showModels() }, at: 1) }
        if !client.isReady, client.process?.isRunning != true, !closing { actions.insert(WorkspaceAction(title: "Reconnect Workspace", shortcut: "", keywords: ["connection", "retry", "recover"]) { [weak self] in self?.reconnect() }, at: 0) }
        if client.isReady, uncertainSubmission != nil, !checkingSubmission { actions.insert(WorkspaceAction(title: "Check Instruction Status", shortcut: "", keywords: ["retry", "submission", "recover"]) { [weak self] in self?.reconcileSubmission() }, at: 0) }
        actions.append(WorkspaceAction(title: "Project Library…", shortcut: "", keywords: ["projects", "open recent"]) { [weak self] in self?.onProjects?() })
        actions.append(WorkspaceAction(title: "ChatGPT Connection…", shortcut: "", keywords: ["account", "sign in", "authentication"]) { [weak self] in self?.onAccount?() })
        actions.append(WorkspaceAction(title: "Sovereign Swarm Radar…", shortcut: "⌥⌘R", keywords: ["radar", "telemetry", "nodes", "edge", "latency", "swarm"]) { SwarmRadarWindow.shared.present() })
        if canOpenFile { actions.insert(WorkspaceAction(title: "Open File…", shortcut: "⌘P") { [weak self] in self?.showQuickOpen() }, at: 1) }
        if client.isReady && (swarmDeferEnabled || swarmDeferred) {
            actions.insert(WorkspaceAction(title: swarmDeferred ? "Resume Agent Work" : "Defer Agent Work", shortcut: "", keywords: ["swarm", "pause", "resume", "safe boundary"]) { [weak self] in self?.toggleAgentDefer() }, at: 1)
        }
        if client.isReady {
            actions.insert(WorkspaceAction(title: "Command Recovery…", shortcut: "", keywords: ["terminal", "command", "reconcile", "uncertain", "receipt"]) { [weak self] in self?.showCommandRecovery() }, at: 1)
        }
        if client.isReady {
            actions.insert(WorkspaceAction(title: "Agent Swarm Settings…", shortcut: "", keywords: ["review", "legion", "defer", "agents"]) { [weak self] in self?.showAgentSwarmSettings() }, at: 1)
            if criticalReviewEnabled {
                actions.insert(WorkspaceAction(title: "Review Latest Completion…", shortcut: "", keywords: ["critical", "adversarial", "audit", "swarm"]) { [weak self] in self?.requestCriticalReview() }, at: 1)
            }
            if legionEnabled { actions.insert(WorkspaceAction(title: "Engage Specialist Team…", shortcut: "", keywords: ["swarm", "legion", "delegate", "agents"]) { [weak self] in self?.engageSpecialists() }, at: 1) }
            actions.insert(WorkspaceAction(title: "Formation Status…", shortcut: "", keywords: ["swarm", "legion", "reports", "reconcile"]) { [weak self] in self?.showFormationStatus() }, at: 1)
            actions.insert(WorkspaceAction(title: "Formation History…", shortcut: "", keywords: ["swarm", "legion", "past reports", "reconcile"]) { [weak self] in self?.showFormationHistory() }, at: 1)
            actions.insert(WorkspaceAction(title: "Inspect Latest Specialist Stream…", shortcut: "", keywords: ["swarm", "legion", "transcript", "stream"]) { [weak self] in self?.showLatestSpecialistStream() }, at: 1)
        }
        if client.capabilities.contains("project-run-v1") { actions.insert(WorkspaceAction(title: "Run Project", shortcut: "⌘R") { [weak self] in self?.showProjectRun() }, at: 1) }
        if client.capabilities.contains("checkpoints-v1") { actions.insert(WorkspaceAction(title: "Checkpoints", shortcut: "⌘7") { [weak self] in self?.showCheckpoints() }, at: 1) }
        if canShowMemory { actions.insert(WorkspaceAction(title: "Conversation Memory…", shortcut: "⌘8", keywords: ["remember", "context", "facts", "preferences", "notes"]) { [weak self] in self?.showMemory() }, at: 1) }
        if canShowContext { actions.insert(WorkspaceAction(title: "Conversation Context", shortcut: "⌘9", keywords: ["history", "summary", "retained", "memory", "inspect"]) { [weak self] in self?.showContext() }, at: 1) }
        if canCheckCode { actions.insert(WorkspaceAction(title: "Check Code", shortcut: "", keywords: ["spider", "syntax", "imports", "cycles", "diagnostics"]) { [weak self] in self?.showCodeCheck() }, at: 1) }
        if canShowSavedState { actions.insert(WorkspaceAction(title: "Saved State", shortcut: "", keywords: ["history", "storage", "integrity", "recovery", "runtime"]) { [weak self] in self?.showSavedState() }, at: 1) }
        if canSwitchConversation { actions.insert(WorkspaceAction(title: "New Conversation", shortcut: "⌘N") { [weak self] in self?.newConversation() }, at: 1) }
        if hasWork { actions.insert(WorkspaceAction(title: "Stop All Work", shortcut: "⌘.") { [weak self] in self?.stop() }, at: 1) }
        if canReviewApproval { actions.insert(WorkspaceAction(title: "Review Request…", shortcut: "⌘5") { [weak self] in self?.showApprovalReview() }, at: 1) }
        if canDecideApproval {
            actions.insert(WorkspaceAction(title: "Allow Operation Once", shortcut: "⇧⌘A", keywords: ["approval", "permission", "allow", "run"]) { [weak self] in self?.allowOnce() }, at: 1)
            actions.insert(WorkspaceAction(title: "Deny Operation", shortcut: "⇧⌘D", keywords: ["approval", "reject", "cancel"]) { [weak self] in self?.deny() }, at: 2)
        }
        if canContinueWork, let continuation = WorkspaceContinuation.latest(in: runs) {
            actions.insert(WorkspaceAction(title: continuation.title, shortcut: "") { [weak self] in self?.continueWork(continuation) }, at: 0)
        }
        if showingInspector { actions.append(WorkspaceAction(title: "Reload Inspected Content", shortcut: "") { [weak self] in self?.refreshInspection() }) }
        if canMaximizeContext { actions.append(WorkspaceAction(title: isContextMaximized ? "Restore Task Context Size" : "Maximize Task Context", shortcut: "⌃⌘M", keywords: ["maximize", "expand", "context", "panel", "preview", "run", "inspect"]) { [weak self] in self?.toggleMaximizeContext() }) }
        if canInspectBack { actions.append(WorkspaceAction(title: "Back in Inspector", shortcut: "⌘[") { [weak self] in self?.inspectBack() }) }
        if canInspectForward { actions.append(WorkspaceAction(title: "Forward in Inspector", shortcut: "⌘]") { [weak self] in self?.inspectForward() }) }
        if canFindInInspection { actions.append(WorkspaceAction(title: "Find in Current Page", shortcut: "⌘F") { [weak self] in self?.findInInspection() }) }
        else if canFindInWalkthrough { actions.append(WorkspaceAction(title: "Filter Walkthrough Activity", shortcut: "⌘F", keywords: ["find", "search", "walkthrough", "activity"]) { [weak self] in self?.findInWalkthrough() }) }
        return actions
    }

    @objc func showActions() {
        guard let window, window.attachedSheet == nil else { return }
        palette?.cancelOperation(nil)
        let actions = buildActions()
        palette = ActionPalette(actions: actions); palette?.present(for: window)
    }

    @objc func showQuickOpen() { presentQuickOpen(mode: .files) }
    @objc func showGoToLine() { presentQuickOpen(mode: .line) }
    @objc func showGoToSymbol() { presentQuickOpen(mode: .symbols) }
    @objc func showCommandPalette() { presentQuickOpen(mode: .actions) }

    func presentQuickOpen(mode: OmnibarMode = .files) {
        guard canOpenFile, let window else { return }
        if let quickOpen, quickOpen.window?.isVisible == true {
            quickOpen.window?.makeKeyAndOrderFront(nil)
            quickOpen.present(for: window, mode: mode)
            return
        }
        let picker = QuickOpenWindow(folder: folder, search: { [weak self] query, refresh, completion in
            guard let self else { return }
            self.client.request("files.search", params: ["query": query, "refresh": refresh], completion: completion)
        }, actions: buildActions(), symbols: { [weak self] in
            guard let self else { return [] }
            let text = self.inspection.string as NSString
            return self.declarations.enumerated().map { index, decl in
                let safeLocation = min(decl.range.location, text.length)
                let prefix = text.substring(to: safeLocation)
                let line = prefix.reduce(1) { $1 == "\n" ? $0 + 1 : $0 }
                let parts = decl.title.split(separator: " ").map(String.init)
                let kind = parts.dropLast().joined(separator: " ").isEmpty ? "Declaration" : parts.dropLast().joined(separator: " ")
                let name = parts.last ?? decl.title
                return (name: name, kind: kind, line: line, index: index)
            }
        }, checkpoints: { [weak self] in
            guard let self else { return [] }
            var seen = Set<String>(); var result = [OmnibarCheckpointItem]()
            for run in self.runs.reversed() {
                let text = String((run["text"] as? String ?? "Instruction").replacingOccurrences(of: "\n", with: " ").prefix(60))
                if let after = run["checkpointAfter"] as? String, !after.isEmpty, seen.insert(after).inserted { result.append(OmnibarCheckpointItem(id: after, title: "After: \(text)", detail: "Checkpoint \(String(after.prefix(8)))")) }
                if let before = run["checkpointBefore"] as? String, !before.isEmpty, seen.insert(before).inserted { result.append(OmnibarCheckpointItem(id: before, title: "Before: \(text)", detail: "Checkpoint \(String(before.prefix(8)))")) }
            }
            return result
        }, lines: { [weak self] in
            guard let self, !self.inspection.string.isEmpty else { return [] }
            return (self.inspection.string as NSString).components(separatedBy: "\n")
        }, open: { [weak self] path in self?.readFile(path) })
        picker.onSelectSymbol = { [weak self] index in
            guard let self, self.declarations.indices.contains(index) else { return }
            self.contextTabs.selectedSegment = 2; self.showingInspector = true; self.surfaces.selectedSegment = 1; self.adapt()
            let range = self.declarations[index].range
            self.inspection.setSelectedRange(range); self.inspection.scrollRangeToVisible(range)
            self.window?.makeFirstResponder(self.inspection)
        }
        picker.onSelectCheckpoint = { [weak self] checkpoint in self?.openCheckpoints(selected: checkpoint) }
        picker.onSelectLine = { [weak self] line, col in self?.goToLine(line, column: col) }
        quickOpen = picker
        quickOpen?.present(for: window, mode: mode)
    }

    var canShowMemory: Bool { client.isReady && client.capabilities.contains("conversation-memory-v1") && !switchingConversation && !closing }
    var canShowContext: Bool { client.isReady && client.capabilities.contains("conversation-context-v1") && !switchingConversation && !closing }
    var canCheckCode: Bool {
        guard client.isReady, client.capabilities.contains("spider-check-v1"), !switchingConversation, !closing, let file = inspectedFile else { return false }
        return ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"].contains((file as NSString).pathExtension.lowercased())
    }

    @objc func showCodeCheck() { if canCheckCode, let file = inspectedFile { inspect(.spider(file)) } }

    @objc func draftCodeReview() {
        guard client.isReady, !switchingConversation, !closing, let details = spiderDetails, case .spider = inspectionHistory.current else { return }
        guard let draft = details.appendingReview(to: composer.string, limit: client.instructionLimit) else { showNotice("Your draft is too long to add this review. Shorten it or copy the findings first."); return }
        composer.string = draft; saveDraft(); focusComposer()
        showNotice("Review request added to your draft. Edit it, then choose Send when ready.")
    }

    @objc func showContext() { if canShowContext, let conversation = currentConversation { inspect(.context(conversation)) } }

    var canShowSavedState: Bool { client.isReady && client.capabilities.contains("runtime-state-v1") && currentConversation != nil && !switchingConversation && !closing }

    @objc func showSavedState() { if canShowSavedState, let conversation = currentConversation { inspect(.savedState(conversation)) } }

    var isMemoryKeyWindow: Bool { memoryWindow?.isKeyWindow == true }
    var canFindMemory: Bool { memoryWindow?.canFocusSearch == true }

    func findMemory() { memoryWindow?.focusSearch() }

    @objc func showMemory() {
        guard canShowMemory, let conversation = currentConversation, let window else { return }
        if let memoryWindow, memoryWindow.window?.isVisible == true {
            if memoryWindow.state.conversation == conversation || memoryWindow.state.dirty || memoryWindow.state.uncertain || memoryWindow.state.saving {
                memoryWindow.present(for: window); return
            }
            memoryWindow.dismiss()
        }
        memoryWindow = ConversationMemoryWindow(folder: folder, conversation: conversation, title: chatTitle.stringValue,
            request: { [weak self] method, params, completion in
                guard let self else { completion(.failure(BridgeError.message("Workspace closed."))); return }
                self.client.request(method, params: params) { [weak self] result in
                    if case .failure(let error) = result, error.localizedDescription.contains("MEMORY_SAVE_UNCERTAIN") { self?.client.closeInput() }
                    completion(result)
                }
            }, available: { [weak self] in self?.canShowMemory == true && self?.currentConversation == conversation },
            writable: { [weak self] in self?.canShowMemory == true && self?.currentConversation == conversation && self?.canRequestRecovery == true })
        memoryWindow?.present(for: window)
    }

    @objc func focusComposer() {
        palette?.cancelOperation(nil)
        quickOpen?.dismiss(restoreFocus: false)
        modelPicker?.dismiss(restoreFocus: false)
        checkpointWindow?.close()
        surfaces.selectedSegment = 0; adapt()
        window?.makeKeyAndOrderFront(nil); window?.makeFirstResponder(composer)
    }

    func textDidChange(_ notification: Notification) {
        draftTimer?.invalidate()
        draftTimer = Timer.scheduledTimer(withTimeInterval: 0.3, repeats: false) { [weak self] _ in self?.saveDraft() }
        updateStatus()
    }

    func submitProbe(_ instruction: String) { composer.string = instruction; submit() }

    func navigateProbe(_ step: String) {
        if step == "memory" { showMemory(); return }
        if step.hasPrefix("memory:") { memoryWindow?.navigateProbe(String(step.dropFirst(7))); return }
        if step.hasPrefix("related:") {
            let kind = String(step.dropFirst("related:".count))
            if let index = evidenceLinks.firstIndex(where: { $0.kind.rawValue == kind }) {
                relatedEvidence.selectItem(at: index + 1); openRelatedEvidence()
            }
            return
        }
        if step.hasPrefix("reasoning:") { reasoningPicker.select(String(step.dropFirst("reasoning:".count))); return }
        if step.hasPrefix("model-draft:") { modelPicker?.editProbe(String(step.dropFirst("model-draft:".count))); return }
        if step.hasPrefix("model-search:") { modelPicker?.searchProbe(String(step.dropFirst("model-search:".count))); return }
        if step.hasPrefix("model-select:") { modelPicker?.selectProbe(String(step.dropFirst("model-select:".count))); return }
        if step.hasPrefix("palette-search:") { palette?.searchProbe(String(step.dropFirst("palette-search:".count))); return }
        if step.hasPrefix("quick-open:") { showQuickOpen(); quickOpen?.searchProbe(String(step.dropFirst("quick-open:".count))); return }
        if step.hasPrefix("walkthrough-") {
            let action = String(step.dropFirst("walkthrough-".count))
            if action == "queue" { submitProbe("Queued fixture instruction: explain the changes.") }
            else if action == "deny" { deny() }
            else if action == "reconnect" { reconnect() }
            else { showWalkthrough(); walkthrough.navigateProbe(action) }
            return
        }
        switch step {
        case "context", "context-empty": showContext()
        case "spider-file": showFiles(); readFile("entry.ts")
        case "saved-state", "saved-state-empty": showSavedState()
        case "saved-state-open": relatedEvidence.selectItem(at: 1); openRelatedEvidence()
        case "saved-state-reload": refreshInspection()
        case "spider": showCodeCheck()
        case "spider-reload": refreshInspection()
        case "spider-review": composer.string = "Keep this existing draft."; draftCodeReview()
        case "spider-open": relatedEvidence.selectItem(at: 1); openRelatedEvidence()
        case "context-reload": refreshInspection()
        case "context-files": showFiles()
        case "context-history":
            if let index = contextLinks.firstIndex(of: .history) { relatedEvidence.selectItem(at: index + 1); openRelatedEvidence() }
        case "context-history-back": inspectBack()
        case "context-history-find": findInInspection()
        case "context-history-next": loadNextPage()
        case "context-history-previous": loadPreviousPage()
        case "context-history-reload": refreshInspection()
        case "context-history-empty": if let conversation = currentConversation { inspect(.contextHistory(conversation)) }
        case "context-memory":
            if let index = contextLinks.firstIndex(of: .memory) { relatedEvidence.selectItem(at: index + 1); openRelatedEvidence() }
        case "context-saved-state":
            if let index = contextLinks.firstIndex(of: .savedState) { relatedEvidence.selectItem(at: index + 1); openRelatedEvidence() }
        case "context-close-memory": memoryWindow?.dismiss(); memoryWindow = nil
        case "files": showFiles(); readFile("fixture.txt")
        case "code", "compact-code": showFiles(); readFile("WorkspaceExample.swift")
        case "walkthrough": showWalkthrough()
        case "checkpoints": showCheckpoints()
        case "checkpoints-compact", "checkpoints-after-restore": showCheckpoints()
        case "checkpoint-review", "checkpoint-review-compact": checkpointWindow?.navigateProbe("review")
        case "checkpoint-flag": checkpointWindow?.navigateProbe("flag")
        case "checkpoint-safety": checkpointWindow?.navigateProbe("safety")
        case "checkpoint-flagged": checkpointWindow?.navigateProbe("flagged")
        case "checkpoint-search": checkpointWindow?.navigateProbe("search:Make my app")
        case "checkpoint-search-empty": checkpointWindow?.navigateProbe("search:no-such-message")
        case "checkpoint-all": checkpointWindow?.navigateProbe("search:"); checkpointWindow?.navigateProbe("all")
        case "checkpoint-restore": checkpointWindow?.navigateProbe("restore")
        case "checkpoint-close": checkpointWindow?.navigateProbe("close"); focusComposer()
        case "checkpoint-link-before": if let id = runs.last?["checkpointBefore"] as? String { openCheckpoints(selected: id) }
        case "checkpoint-link-after": if let id = runs.last?["checkpointAfter"] as? String { openCheckpoints(selected: id) }
        case "project": showProjectRun()
        case "project-start": projectRun.navigateProbe("start")
        case "project-stop": projectRun.navigateProbe("stop")
        case "project-automatic": projectRun.navigateProbe("automatic")
        case "project-repair", "project-view-repair": projectRun.navigateProbe("repair")
        case "project-details": projectRun.navigateProbe("details")
        case "project-configure": projectRun.navigateProbe("configure")
        case "project-draft": projectRun.navigateProbe("draft")
        case "project-refresh": projectRun.navigateProbe("refresh")
        case "project-configure-scroll": projectRun.navigateProbe("configuration-end")
        case "project-configure-save": projectRun.navigateProbe("save")
        case "quick-open": showQuickOpen()
        case "quick-open-select", "quick-open-select-compact": quickOpen?.openSelectedProbe()
        case "quick-open-close": quickOpen?.dismiss()
        case "quick-open-focus": focusComposer()
        case "conversation": focusComposer()
        case "conversation-new": newConversation()
        case "conversation-sidebar": toggleNavigation()
        case "task-context": surfaces.selectedSegment = 1; switchSurface()
        case "conversation-delete-sheet", "conversation-delete-sheet-compact": if let id = currentConversation { confirmDeleteConversation(id) }
        case "conversation-delete-cancel", "conversation-delete-cancel-compact": if let sheet = window?.attachedSheet { window?.endSheet(sheet, returnCode: NSApplication.ModalResponse.alertFirstButtonReturn) }
        case "conversation-delete-confirm": if let sheet = window?.attachedSheet { window?.endSheet(sheet, returnCode: NSApplication.ModalResponse.alertSecondButtonReturn) }
        case "new-conversation": newConversation()
        case "previous-conversation": if let id = conversationList.last?.id { selectConversation(id) }
        case "models": showModels()
        case "models-close": modelPicker?.dismiss()
        case "models-reopen": showModels()
        case "models-refresh", "models-refresh-retry": modelPicker?.refreshProbe()
        case "models-claude": modelPicker?.providerProbe("claude-subscription-directsdk-experimental")
        case "models-openrouter": modelPicker?.providerProbe("openrouter")
        case "models-cline", "models-cline-pass": modelPicker?.providerProbe("cline-pass")
        case "models-nous", "models-nous-research": modelPicker?.providerProbe("nous")
        case "models-opencode": modelPicker?.providerProbe("opencode")
        case "models-apple-playground", "models-local", "models-playground": modelPicker?.providerProbe("apple-playground")
        case "models-openai": modelPicker?.providerProbe("openai-codex")
        case "models-disconnect": client.closeInput()
        case "models-reconnect": reconnect()
        case "palette-close": palette?.cancelOperation(nil)
        case "draft", "draft-light": composer.string = "Keep the conversation in focus as you refine the sidebar."; updateStatus()
        case "clear-draft", "clear-draft-light": composer.string = ""; updateStatus()
        case "expanded", "expanded-light":
            if let activity = rows.first(where: { $0.activity }) { expandedActions.insert(activity.id); rebuildRows() }
        case "collapsed", "collapsed-light": expandedActions.removeAll(); rebuildRows()
        case "wide":
            if let window, let screen = window.screen ?? NSScreen.main {
                let available = screen.visibleFrame
                let size = NSSize(width: min(1440, available.width), height: min(880, available.height))
                window.setFrame(NSRect(x: available.midX - size.width / 2, y: available.maxY - size.height, width: size.width, height: size.height), display: true)
            }
            surfaces.selectedSegment = 1; adapt()
        case "changes": showChanges(); diffScope.selectedSegment = 0; readDiff("fixture.txt")
        case "sessions":
            showSessions()
            if let id = runs.first(where: { $0["state"] as? String == "cancelled" })?["id"] as? String { inspect(.run(id)) }
        case "run-output":
            if let id = runs.first(where: { $0["state"] as? String == "completed" })?["id"] as? String { inspect(.run(id)) }
        case "output": openRecordedOutput()
        case "back": inspectBack()
        case "forward": inspectForward()
        case "find": findInInspection()
        case "missing-file": readFile("missing.txt")
        case "narrow":
            if let window { window.setFrame(NSRect(origin: window.frame.origin, size: NSSize(width: 720, height: 480)), display: true) }
            compactSidebarRevealed = false; adapt()
        case "dark": window?.appearance = NSAppearance(named: .darkAqua)
        case "light": window?.appearance = NSAppearance(named: .aqua)
        case "palette": showActions()
        case "queue": showQueue()
        case "approval", "approval-wide", "approval-light", "approval-dark": showApprovalReview()
        case "approval-close": approvalReview?.cancelOperation(nil)
        case "queue-edit":
            if let run = runs.first(where: { $0["state"] as? String == "queued" }), let id = run["id"] as? String {
                editQueued(QueuedInstruction(id: id, text: run["text"] as? String ?? "", revision: run["revision"] as? Int ?? 0))
            }
        default: break
        }
        modelPicker?.syncPresentation()
    }

    var probeLayout: [String: Any] {
        let bounds = NSRect(origin: .zero, size: window?.frame.size ?? .zero)
        var controls = [String: Any]()
        for (name, view) in [("transcript", transcriptScroll as NSView), ("jumpLatest", jumpButton), ("model", modelButton), ("reasoning", reasoningPicker), ("composer", composer.enclosingScrollView! as NSView), ("send", sendButton), ("queue", queueButton), ("stop", stopButton), ("reviewApproval", reviewApprovalButton), ("deny", denyButton), ("inspection", inspectionScroll), ("inspectionBack", inspectionBack), ("inspectionForward", inspectionForward), ("inspectionFind", inspectionFind), ("recordedOutput", recordedOutput), ("relatedEvidence", relatedEvidence)] {
            if !view.isHiddenOrHasHiddenAncestor {
                let frame = view.convert(view.bounds, to: nil as NSView?)
                controls[name] = ["visibleWithinWindow": bounds.contains(frame), "x": frame.minX, "y": frame.minY, "width": frame.width, "height": frame.height]
            }
        }
        return ["messageLayoutIssues": (0..<rows.count).compactMap { (transcript.view(atColumn: 0, row: $0, makeIfNecessary: false) as? TranscriptMessageCell)?.layoutIssue }, "messageContentFits": (0..<rows.count).compactMap { transcript.view(atColumn: 0, row: $0, makeIfNecessary: false) as? TranscriptMessageCell }.allSatisfy { $0.contentFits }, "selectedSurface": surfaces.selectedSegment, "contextVisible": !contextPanel.isHidden, "conversationSidebarVisible": !conversations.isHidden, "conversationCount": conversationList.count, "deleteSheetVisible": window?.attachedSheet != nil, "projectRun": projectRun.diagnostics, "memory": memoryWindow?.diagnostics ?? [:], "checkpoints": checkpointWindow?.diagnostics ?? [:], "messageRoles": rows.map { $0.user ? "user" : $0.activity ? "activity" : "agent" }, "quickOpen": quickOpen?.diagnostics ?? [:], "modelPicker": modelPicker?.diagnostics ?? [:], "reasoningPicker": reasoningPicker.diagnostics, "actionPalette": palette?.diagnostics ?? [:], "workspaceKey": window?.isKeyWindow == true, "composerFocused": window?.firstResponder === composer, "conversation": currentConversation ?? "", "switchingConversation": switchingConversation, "walkthrough": walkthrough.diagnostics, "windowFrameWidth": bounds.width, "windowFrameHeight": bounds.height, "controls": controls, "destination": navigation.destination.title, "navigationVisible": !navigation.isHidden, "inspectionPreview": String(inspection.string.prefix(240)), "inspectionTitle": inspectorTitle.stringValue, "inspectionPage": pageLabel.stringValue, "canInspectBack": canInspectBack, "canInspectForward": canInspectForward, "recordedOutputAvailable": !recordedOutput.isHidden, "relatedEvidence": evidenceLinks.map { ["kind": $0.kind.rawValue, "id": $0.id, "label": $0.label] }, "findBarVisible": inspectionScroll.isFindBarVisible, "appearance": window?.effectiveAppearance.name.rawValue ?? "system", "forceStopAvailable": forceAvailable, "composerUTF16Count": composer.string.utf16.count, "draftSaveFailed": draftSaveFailed, "submissionNeedsReconciliation": uncertainSubmission != nil]
    }

    func mergeRun(_ run: [String: Any]) {
        guard let id = run["id"] as? String else { return }
        if let index = runs.firstIndex(where: { $0["id"] as? String == id }) {
            if let incoming = run["revision"] as? Int, let current = runs[index]["revision"] as? Int, incoming < current { return }
            runs[index] = WalkthroughProjection.merged(runs[index], run)
        } else { runs.append(WalkthroughProjection.bounded(run)) }
        trimRecordedRuns()
    }

    func trimRecordedRuns() {
        let retained = Set(runs.filter { terminalStates.contains($0["state"] as? String ?? "") }.suffix(12).compactMap { $0["id"] as? String })
        runs.removeAll { terminalStates.contains($0["state"] as? String ?? "") && !retained.contains($0["id"] as? String ?? "") }
    }

    func receive(_ event: [String: Any]) {
        guard let payload = event["payload"] as? [String: Any] else { return }
        if event["kind"] as? String == "snapshot" {
            if switchingConversation && (expectedConversation == nil || payload["conversation"] as? String != expectedConversation) { return }
            guard (payload["seq"] as? Int ?? 0) >= sequence else { return }
            if let conversation = payload["conversation"] as? String { adoptConversation(conversation) }
            if let data = payload["modelSelection"] as? [String: Any], let selection = WorkspaceModelSelection(data) { updateModel(selection); modelPicker?.receive(data) }
            sequence = payload["seq"] as? Int ?? 0
            let incoming = payload["runs"] as? [[String: Any]] ?? []
            let previous = Dictionary(runs.compactMap { run in (run["id"] as? String).map { ($0, run) } }, uniquingKeysWith: { _, newest in newest })
            runs = incoming.map { run in previous[run["id"] as? String ?? ""].map { WalkthroughProjection.merged($0, run) } ?? WalkthroughProjection.bounded(run) }
            trimRecordedRuns()
            walkthroughLoaded = nil; rebuildRows()
            renderConversations()
            finishStopAndSend()
            if probe?.instruction == nil, let state = runs.last?["state"] as? String, terminalStates.contains(state) { probe?.outcome(self, state: state) }
            return
        }
        if let seq = event["seq"] as? Int {
            if seq <= sequence { return }
            if seq != sequence + 1 { client.requestSnapshot(); return }
            sequence = seq
        }
        if event["type"] as? String == "models.changed", let selection = WorkspaceModelSelection(payload) { updateModel(selection); modelPicker?.receive(payload); return }
        if event["type"] as? String == "project.state" { projectRun.receiveProjectState(payload); return }
        if event["type"] as? String == "project.tasks_updated", let revision = payload["revision"] as? Int {
            if revision > projectTaskRevision {
                projectTaskRevision = revision; projectTaskOffset = 0; projectTaskPreviousOffsets = []; refreshProjectTasks()
            }
            return
        }
        if ["activity", "activity.batch"].contains(event["type"] as? String ?? ""), let runID = event["run"] as? String, let index = runs.firstIndex(where: { $0["id"] as? String == runID }) {
            let values = event["type"] as? String == "activity.batch" ? payload["activities"] as? [[String: Any]] ?? [] : [payload]
            let previousSequence = runs[index]["activitySequence"] as? Int ?? 0
            guard WalkthroughProjection.apply(values, to: &runs[index]) else { return }
            if values.contains(where: { activity in
                let metadata = activity["metadata"] as? [String: Any]
                return (activity["sequence"] as? Int ?? 0) > previousSequence && ["completed", "failed"].contains(activity["status"] as? String ?? "")
                    && (activity["phase"] as? String == "writing" || metadata?["itemType"] as? String == "run_command")
            }) { scheduleWorkspaceRefresh() }
            if let count = payload["activityCount"] as? Int { runs[index]["activityCount"] = max(count, runs[index]["activityCount"] as? Int ?? 0) }
            needsRebuild = true
            if window?.isMiniaturized == true || window?.occlusionState.contains(.visible) != true { return }
            if layoutTimer == nil { layoutTimer = Timer.scheduledTimer(withTimeInterval: window?.isKeyWindow == true ? 0.1 : 1, repeats: false) { [weak self] _ in self?.layoutTimer = nil; self?.rebuildRows() } }
        } else if RuntimeClient.runEvents.contains(event["type"] as? String ?? ""), payload["id"] is String {
            if let conversation = payload["conversation"] as? String, conversation != currentConversation { return }
            mergeRun(payload); rebuildRows(); finishStopAndSend()
            if event["type"] as? String == "run.accepted" { walkthrough.followCurrentInstruction(); loadConversations() }
            if let state = payload["state"] as? String, terminalStates.contains(state) { scheduleWorkspaceRefresh(); probe?.outcome(self, state: state) }
        } else if event["type"] as? String == "runtime.error" { showNotice(payload["message"] as? String ?? "Runtime state is unknown") }
    }

    func rebuildRows() {
        needsRebuild = false
        let following = transcriptScroll.documentVisibleRect.maxY >= transcript.bounds.height - 35
        let origin = transcriptScroll.contentView.bounds.origin
        var next = [TranscriptRow]()
        active = nil; currentApproval = nil
        for run in runs {
            guard let id = run["id"] as? String else { continue }
            let state = run["state"] as? String ?? "unknown"
            let automatic = run["origin"] as? String == "automatic_repair"
            next.append(TranscriptRow(id: "\(id)-user", title: automatic ? "LUMI · Automatic repair" : state == "queued" ? "You · Queued" : "You", text: run["text"] as? String ?? "", activity: false, user: !automatic, checkpoint: run["checkpointBefore"] as? String, automatic: automatic, runID: id))
            let activities = run["activities"] as? [[String: Any]] ?? []
            let parsedActivities = WalkthroughProjection.activities(run)
            let visibleActivities = activities.filter { ($0["metadata"] as? [String: Any])?["scope"] as? String != "turn" }
            let runIsLive = ["running", "cancelling", "awaiting_approval"].contains(state)
            let summaryID = "\(id)-activity"
            if !activities.isEmpty {
                let swarmWorkers = visibleActivities.filter {
                    let meta = $0["metadata"] as? [String: Any]
                    let kind = $0["kind"] as? String ?? ""
                    let msg = $0["message"] as? String ?? ""
                    return kind.contains("swarm") || kind.contains("worker") || meta?["swarm"] != nil || meta?["worker"] != nil || msg.hasPrefix("Worker ") || msg.hasPrefix("Subagent ")
                }
                let latestRaw = activities.max(by: { ($0["sequence"] as? Int ?? 0) < ($1["sequence"] as? Int ?? 0) })
                let latestParsed = parsedActivities.filter { $0.isActive && $0.scope != "turn" }.max { $0.sequence < $1.sequence } ?? parsedActivities.max { $0.sequence < $1.sequence }
                let latest = latestParsed?.displayTitle ?? (latestRaw?["message"] as? String ?? "Recorded activity")
                let count = visibleActivities.count
                let coverage = (run["activityCount"] as? Int ?? activities.count) > activities.count ? "loaded" : "recorded"
                let swarmBadge = swarmWorkers.isEmpty ? "" : " [Swarm: \(swarmWorkers.count) updates]"
                let progress = WalkthroughProjection.progressSummary(parsedActivities, live: runIsLive)
                let latestTurnDetail = parsedActivities.filter { $0.scope == "turn" && !$0.detail.isEmpty }.max { $0.sequence < $1.sequence }?.detail
                let activeIdleNote = (runIsLive ? latestParsed : nil).flatMap { item -> String? in
                    guard item.isActive else { return nil }
                    var parts = [String]()
                    if let attempt = item.attempt, attempt > 1 { parts.append("Attempt \(attempt)") }
                    if let idle = item.outputIdleMs, idle >= 5000 { parts.append("Quiet \(idle / 1000)s") }
                    return parts.isEmpty ? nil : parts.joined(separator: " · ")
                }
                let summaryEvidence = [progress, latestTurnDetail ?? "", activeIdleNote ?? ""].filter { !$0.isEmpty }.joined(separator: " · ")
                let summaryTitle = count == 0 && runIsLive ? "Activity · \(latestParsed?.stage ?? "Starting") · \(latest)" : "Activity · \(count) \(coverage)\(swarmBadge) · \(latest)"
                next.append(TranscriptRow(id: summaryID, title: summaryTitle, text: expandedActions.contains(summaryID) ? "Details below · code, commands and results are in Walkthrough" : "", activity: true, evidence: summaryEvidence.isEmpty ? nil : summaryEvidence, runID: id, actionTitle: expandedActions.contains(summaryID) ? "Open in Walkthrough" : nil))
            }
            for activity in expandedActions.contains(summaryID) ? visibleActivities : [] {
                let rawID = activity["activityId"] as? String ?? ""
                let aid = "\(id)-\(rawID)"
                let parsed = WalkthroughActivity(activity)
                let phase = (activity["status"] as? String ?? "").replacingOccurrences(of: "_", with: " ")
                let elapsed = (activity["elapsedMs"] as? Double).map { String(format: " · %.1fs", $0 / 1000) } ?? ""
                let meta = activity["metadata"] as? [String: Any]
                let workerTag = (meta?["worker"] as? String).map { "[\($0)] " } ?? ""
                let captionSuffix: String
                if let parsed {
                    var extras = [String]()
                    if let attempt = parsed.attempt, attempt > 1 { extras.append("Attempt \(attempt)") }
                    if let code = parsed.exitCode { extras.append("Exit \(code)") }
                    if let bytes = parsed.outputBytes { extras.append("\(ByteCountFormatter.string(fromByteCount: Int64(bytes), countStyle: .file)) received") }
                    if let lines = parsed.outputLines, lines > 0 { extras.append("\(lines) \(lines == 1 ? "line" : "lines")") }
                    if parsed.isActive, runIsLive, let idle = parsed.outputIdleMs, idle >= 5000 { extras.append("Quiet \(idle / 1000)s") }
                    captionSuffix = extras.isEmpty ? "" : " · " + extras.joined(separator: " · ")
                } else { captionSuffix = "" }
                let rowEvidence = parsed.flatMap { $0.files.isEmpty ? nil : $0.files.joined(separator: " · ") }
                var expandedText = ""
                if expandedActions.contains(aid) {
                    if let parsed {
                        expandedText = parsed.presentedDetail(live: runIsLive, connected: client.isReady, paused: state == "awaiting_approval")
                    } else {
                        expandedText = activity["detail"] as? String ?? "No additional display detail. Recorded output is available in Sessions after the run."
                    }
                }
                let childTitle = parsed?.displayTitle ?? (activity["message"] as? String ?? "Activity")
                next.append(TranscriptRow(id: aid, title: "\(workerTag)\(childTitle) · \(phase)\(elapsed)\(captionSuffix)", text: expandedText, activity: true, evidence: expandedActions.contains(aid) ? rowEvidence : nil, runID: id, activityID: rawID.isEmpty ? nil : rawID, actionTitle: expandedActions.contains(aid) ? "Open in Walkthrough" : nil))
            }
            if let response = run["response"] as? String {
                let route = run["modelSelection"] as? [String: Any]
                let model = route?["model"] as? String
                let reviewDecision = run["criticalReviewDecision"] as? String
                let reviewEvidence: String
                if run["review"] as? String == "unavailable" { reviewEvidence = "Critical review unavailable" }
                else if let reviewDecision { reviewEvidence = "Critical review · \(reviewDecision)" }
                else { reviewEvidence = "Not reviewed" }
                let evidence = WorkspaceVerificationPresentation.label(run) + (model.map { " · Model: " + $0 } ?? "") + (run["executionAuthority"] as? String == "autonomous" ? " · Autonomous" : "") + " · \(reviewEvidence)"
                var resultText = response
                if run["reviewOf"] == nil, run["review"] as? String == "completed", let report = run["criticalReviewReport"] as? String,
                   !(response.contains("Critical completion review requires action")) {
                    resultText += "\n\nCritical completion review · \(reviewDecision ?? "Decision unavailable")\n\(report)"
                }
                if run["reviewOf"] == nil, let tasks = run["criticalReviewCorrectiveTasks"] as? [[String: Any]], !tasks.isEmpty {
                    let lines = tasks.prefix(8).map { task in
                        let priority = task["priority"] as? String ?? "medium"
                        let title = task["title"] as? String ?? "Corrective task"
                        let specialist = task["recommendedSpecialist"] as? String ?? "legion_implementer"
                        let description = task["description"] as? String ?? ""
                        return "• [\(priority.capitalized)] \(title) · \(specialist)\n  \(description)"
                    }
                    resultText += "\n\nCorrective tasks\n" + lines.joined(separator: "\n")
                }
                if run["reviewOf"] == nil,
                   let roadmap = run["criticalReviewRoadmap"] as? [String: Any],
                   let formation = roadmap["remedialFormation"] as? [String: Any] {
                    let status = formation["status"] as? String ?? "recorded"
                    let outstanding = (formation["undispatchedTaskIds"] as? [String] ?? []).count
                    resultText += "\n\nRemedial specialist formation · \(status.replacingOccurrences(of: "_", with: " "))"
                    if outstanding > 0 { resultText += " · \(outstanding) task(s) remain in the roadmap" }
                }
                next.append(TranscriptRow(id: "\(id)-result", title: "LUMI · \(state.capitalized)", text: resultText, activity: false, checkpoint: run["checkpointAfter"] as? String, evidence: evidence))
            }
            if let error = run["checkpointError"] as? String { next.append(TranscriptRow(id: "\(id)-checkpoint-error", title: "Checkpoint unavailable", text: error, activity: false)) }
            if !terminalStates.contains(state), state != "queued" { active = id }
            if state == "awaiting_approval", let a = run["approval"] as? [String: Any], a["decision"] == nil, let operation = a["operation"] as? String {
                currentApproval = ApprovalRequest(run: id, operation: operation, tool: a["tool"] as? String ?? "Operation", scope: a["scope"] as? String ?? folder.path, reason: a["reason"] as? String ?? "", arguments: a["args"] as? String ?? "")
            }
        }
        for query in sideQuestions where query.conversation == currentConversation {
            next.append(TranscriptRow(id: "\(query.id)-side-user", title: "You · Side question", text: query.question, activity: false, user: true))
            let title = query.answer == nil ? "LUMI · Answering" : query.failed ? "Side question failed" : "LUMI · Side answer"
            let detail = query.answer == nil ? "Answering without tools. You can continue the conversation." : query.answer ?? ""
            let evidence = query.contextLimited ? "Temporary · recent context only · no tools" : "Temporary · not added to conversation context · no tools"
            next.append(TranscriptRow(id: "\(query.id)-side-answer", title: title, text: detail, activity: false, evidence: evidence))
        }
        next = Array(next.suffix(200))
        var bytes = next.reduce(0) { $0 + $1.text.utf8.count + $1.title.utf8.count }
        while bytes > 8 * 1024 * 1024, !next.isEmpty { let removed = next.removeFirst(); bytes -= removed.text.utf8.count + removed.title.utf8.count }
        expandedActions.formIntersection(Set(next.map(\.id)))
        if next.isEmpty { next = [welcomeRow] }
        if !next.isEmpty {
            let old = rows; rows = next
            if old.map(\.id) == next.map(\.id) {
                let changed = IndexSet(next.indices.filter { next[$0] != old[$0] })
                if !changed.isEmpty { transcript.reloadData(forRowIndexes: changed, columnIndexes: IndexSet(integer: 0)); transcript.noteHeightOfRows(withIndexesChanged: changed) }
            } else if next.count > old.count, Array(next.prefix(old.count)) == old {
                transcript.insertRows(at: IndexSet(integersIn: old.count..<next.count), withAnimation: [])
            } else { transcript.reloadData() }
            if following { jumpLatest() }
            else { transcriptScroll.contentView.scroll(to: origin); transcriptScroll.reflectScrolledClipView(transcriptScroll.contentView); jumpButton.isHidden = runs.isEmpty }
        }
        var turnItems = [TurnScrubberRail.TurnItem]()
        var turnCounter = 0
        let runsByID = Dictionary(runs.compactMap { r in (r["id"] as? String).map { ($0, r) } }, uniquingKeysWith: { _, latest in latest })
        for (idx, row) in rows.enumerated() where row.user || row.id == "welcome" {
            let run = row.runID.flatMap { runsByID[$0] }
            let state = run?["state"] as? String ?? ""
            let isActive = row.runID != nil && row.runID == active
            let isWaitingReview = state == "awaiting_approval"
            let parsed = run.map(WalkthroughProjection.activities) ?? []
            let isError = ["failed", "interrupted", "timed_out"].contains(state) || parsed.contains(where: \.isIssue)
            let stateBadge = state.isEmpty ? "" : " · \(state == "awaiting_approval" ? "Needs review" : state.replacingOccurrences(of: "_", with: " ").capitalized)"
            let preview = "Turn \(turnCounter + 1)\(stateBadge): \(String(row.text.prefix(80)))"
            turnItems.append(TurnScrubberRail.TurnItem(index: turnCounter, rowIndex: idx, preview: preview, isActive: isActive, isError: isError, isWaitingReview: isWaitingReview))
            turnCounter += 1
        }
        turnScrubber.turns = turnItems
        updateWalkthrough(); updateStatus()
    }
}

