// [LAYER: UI]
import AppKit
import Foundation

extension WorkspaceWindow {
    func buildContextPanel() {
        contextPanel.orientation = .vertical; contextPanel.alignment = .leading; contextPanel.spacing = 10
        contextPanel.edgeInsets = NSEdgeInsets(top: 20, left: 14, bottom: 16, right: 14)
        let title = JourneyStyle.label("Task context", size: 16, weight: .semibold)
        openFileButton.target = self; openFileButton.action = #selector(showQuickOpen); openFileButton.bezelStyle = .rounded
        openFileButton.toolTip = "Open a workspace file by name or path (⌘P) · Command Palette (⌘K)"
        let heading = stack([title, NSView(), openFileButton])
        contextTabs.target = self; contextTabs.action = #selector(changeContext); contextTabs.selectedSegment = 0
        contextTabs.setAccessibilityLabel("Task context: Walkthrough, Changes, Inspect or Run")
        browseContext.target = self; browseContext.action = #selector(toggleContextBrowser); browseContext.bezelStyle = .rounded
        walkthrough.onSelection = { [weak self] _ in self?.loadWalkthrough() }
        walkthrough.onOpenFile = { [weak self] path in self?.readFile(path) }
        walkthrough.onChanges = { [weak self] in self?.showChanges() }
        walkthrough.onOutput = { [weak self] id in self?.inspect(.run(id)) }
        walkthrough.onMoveProjectTask = { [weak self] task, status in self?.moveProjectTask(task, status: status) }
        walkthrough.onPreviousTaskPage = { [weak self] in
            guard let self, let previous = self.projectTaskPreviousOffsets.popLast() else { return }
            self.projectTaskOffset = previous; self.refreshProjectTasks()
        }
        walkthrough.onNextTaskPage = { [weak self] next in
            guard let self, next > self.projectTaskOffset else { return }
            self.projectTaskPreviousOffsets.append(self.projectTaskOffset); self.projectTaskOffset = next; self.refreshProjectTasks()
        }
        walkthrough.onProjectTaskFilters = { [weak self] filters in
            guard let self, self.projectTaskFilters != filters else { return }
            self.projectTaskFilters = filters; self.projectTaskOffset = 0; self.projectTaskPreviousOffsets = []; self.refreshProjectTasks()
        }
        walkthrough.onReview = { [weak self] in self?.showApprovalReview() }
        walkthrough.onRetryLoad = { [weak self] in self?.walkthroughLoaded = nil; self?.loadWalkthrough() }
        walkthrough.onContinue = { [weak self] continuation in self?.continueWork(continuation) }
        let rule = JourneyStyle.rule()
        projectRun.onState = { [weak self] in
            self?.checkpointWindow?.refreshAvailability()
            self?.adapt()
        }
        projectRun.onRepair = { [weak self] launch in self?.repairProject(launch) }
        projectRun.onShowRepair = { [weak self] run in self?.showProjectRepair(run) }
        projectRunScroll.documentView = projectRun; projectRunScroll.hasVerticalScroller = true; projectRunScroll.drawsBackground = false
        projectRun.widthAnchor.constraint(equalTo: projectRunScroll.contentView.widthAnchor).isActive = true
        projectRun.heightAnchor.constraint(greaterThanOrEqualTo: projectRunScroll.contentView.heightAnchor).isActive = true
        let fittingHeight = projectRun.heightAnchor.constraint(equalTo: projectRunScroll.contentView.heightAnchor); fittingHeight.priority = .defaultLow; fittingHeight.isActive = true
        projectRunScroll.setContentHuggingPriority(.init(1), for: .vertical)
        [heading, contextTabs, rule, walkthrough, inspector, projectRunScroll].forEach(contextPanel.addArrangedSubview)
        for view in contextPanel.arrangedSubviews { view.widthAnchor.constraint(equalTo: contextPanel.widthAnchor, constant: -28).isActive = true }
        walkthrough.setContentHuggingPriority(.init(1), for: .vertical)
        inspector.setContentHuggingPriority(.init(1), for: .vertical)
    }

    @objc func stepPosture() {
        applyPosture(activePosture.stepNext(), syncRemote: true)
    }

    func applyPosture(_ stop: ExecutionPostureStop, syncRemote: Bool = true) {
        activePosture = stop
        refreshPostureControl()
        updateStatus()
        guard syncRemote, client.isReady, !closing else { return }
        client.request("posture.set", params: ["posture": stop.rawValue]) { [weak self] result in
            guard let self else { return }
            if case .success(let data) = result,
               let remoteSlug = data["posture"] as? String,
               let parsed = ExecutionPostureStop.parse(remoteSlug),
               parsed != self.activePosture {
                self.activePosture = parsed
                self.refreshPostureControl()
                self.updateStatus()
            }
        }
    }

    func refreshPostureControl() {
        postureButton.title = "Posture: \(activePosture.label)"
        postureButton.image = NSImage(systemSymbolName: activePosture.symbolName, accessibilityDescription: activePosture.label)
        postureButton.imagePosition = .imageLeading
        postureButton.toolTip = activePosture.summaryHint
        postureButton.setAccessibilityLabel("Execution posture \(activePosture.label). Press Shift Tab to cycle.")
    }

    @objc func allowForSession() {
        if let request = currentApproval {
            resolveApproval(request, decision: "allow_session")
        }
    }

    @objc func undoLastTurn() {
        guard client.isReady, !hasWork, !closing, !switchingConversation else {
            showNotice("Finish or stop active work before undoing the last turn.")
            return
        }
        client.request("turn.undo", params: ["force": false]) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let data):
                if let err = data["error"] as? String, !err.isEmpty {
                    self.showNotice("Undo blocked to protect manual edits: \(err)")
                } else {
                    let count = (data["restoredCount"] as? Int) ?? (data["restored_count"] as? Int) ?? (data["restoredFiles"] as? [String])?.count ?? 0
                    self.receiptBar.isHidden = true
                    self.showNotice(count > 0 ? "Reverted \(count) file\(count == 1 ? "" : "s") from the last turn." : "No turn file mutations needed reverting.")
                    self.refreshNavigation()
                    if let file = self.inspectedFile { self.inspect(.file(file), addToHistory: false) }
                }
            case .failure(let error):
                self.showNotice(error.localizedDescription)
            }
        }
    }

    @objc func showRequestPreview() {
        guard client.isReady, !switchingConversation, !closing else { return }
        inspect(.requestPreview)
    }

    @objc func showTurnReceipts() {
        guard client.isReady, !switchingConversation, !closing else { return }
        inspect(.receipts)
    }

    func buildActions() -> [WorkspaceAction] {
        var actions = [
            WorkspaceAction(title: "Cycle Execution Posture (\(activePosture.label) → \(activePosture.stepNext().label))", shortcut: "⇧⇥", keywords: ["posture", "mode", "plan", "ask", "accept edits", "auto", "full access", "shift tab"]) { [weak self] in self?.stepPosture() },
            WorkspaceAction(title: "Open Workspace…", shortcut: "⌘O", keywords: ["folder", "project"]) { NSApp.sendAction(#selector(ApplicationDelegate.openFolder), to: NSApp.delegate, from: nil) },
            WorkspaceAction(title: "Focus Composer", shortcut: "⌘L", keywords: ["input", "prompt", "message"]) { [weak self] in self?.focusComposer() },
            WorkspaceAction(title: "Show Chats", shortcut: "⇧⌘C", keywords: ["chat", "conversations", "threads", "sidebar"]) { [weak self] in self?.showChats() },
            WorkspaceAction(title: "Show Files", shortcut: "⌘1", keywords: ["code", "tree", "explorer"]) { [weak self] in self?.showFiles() },
            WorkspaceAction(title: "Show Changes", shortcut: "⌘2", keywords: ["diff", "git", "modified"]) { [weak self] in self?.showChanges() },
            WorkspaceAction(title: "Show Recorded Runs", shortcut: "⌘3", keywords: ["history", "sessions"]) { [weak self] in self?.showSessions() },
            WorkspaceAction(title: "Show Walkthrough", shortcut: "⌘6", keywords: ["activity", "timeline", "steps"]) { [weak self] in self?.showWalkthrough() },
            WorkspaceAction(title: "Show Up Next", shortcut: "⌘4", keywords: ["queue", "pending instructions"]) { [weak self] in self?.showQueue() },
            WorkspaceAction(title: "Toggle Navigation", shortcut: "⌘B", keywords: ["sidebar", "panel"]) { [weak self] in self?.toggleNavigation() },
            WorkspaceAction(title: "Go to Line…", shortcut: "⌘G", keywords: ["jump", "line", "offset"]) { [weak self] in self?.showGoToLine() },
            WorkspaceAction(title: "Go to Symbol in File…", shortcut: "⇧⌘O", keywords: ["symbol", "declaration", "method", "function", "outline"]) { [weak self] in self?.showGoToSymbol() },
            WorkspaceAction(title: "Command Palette…", shortcut: "⌘K", keywords: ["commands", "palette", "omnibar", "actions"]) { [weak self] in self?.showCommandPalette() },
            WorkspaceAction(title: "Refresh Current Destination", shortcut: "", keywords: ["reload"]) { [weak self] in self?.refreshNavigation() }
        ]
        for stop in ExecutionPostureStop.allCases where stop != activePosture {
            actions.append(WorkspaceAction(title: "Set Posture: \(stop.label)", shortcut: "", keywords: ["posture", "mode", stop.rawValue, stop.label.lowercased()]) { [weak self] in
                self?.applyPosture(stop, syncRemote: true)
            })
        }
        if reasoningPicker.isEnabled { actions.insert(WorkspaceAction(title: "Change Reasoning Level…", shortcut: "", keywords: ["effort", "thinking", "speed", "depth"]) { [weak self] in self?.reasoningPicker.performClick(nil) }, at: 1) }
        if client.capabilities.contains("model-selection-v1") { actions.insert(WorkspaceAction(title: "Choose Model & Provider…", shortcut: "F3 / ⇧⌘M", keywords: ["switch", "change", "AI", "ollama", "anthropic", "openai", "f3"]) { [weak self] in self?.showModels() }, at: 1) }
        if !client.isReady, client.process?.isRunning != true, !closing { actions.insert(WorkspaceAction(title: "Reconnect Workspace", shortcut: "", keywords: ["connection", "retry", "recover"]) { [weak self] in self?.reconnect() }, at: 0) }
        if client.isReady, uncertainSubmission != nil, !checkingSubmission { actions.insert(WorkspaceAction(title: "Check Instruction Status", shortcut: "", keywords: ["retry", "submission", "recover"]) { [weak self] in self?.reconcileSubmission() }, at: 0) }
        actions.append(WorkspaceAction(title: "Project Library…", shortcut: "", keywords: ["projects", "open recent"]) { [weak self] in self?.onProjects?() })
        actions.append(WorkspaceAction(title: "ChatGPT Connection…", shortcut: "", keywords: ["account", "sign in", "authentication"]) { [weak self] in self?.onAccount?() })
        actions.append(WorkspaceAction(title: "Sovereign Swarm Radar…", shortcut: "⌥⌘R", keywords: ["radar", "telemetry", "nodes", "edge", "latency", "swarm"]) { SwarmRadarWindow.shared.present() })
        if canOpenFile { actions.insert(WorkspaceAction(title: "Open File…", shortcut: "⌘P", keywords: ["find", "search", "quick open"]) { [weak self] in self?.showQuickOpen() }, at: 1) }
        if client.isReady {
            actions.insert(WorkspaceAction(title: "Preview Outbound Request & KV-Cache Manifest", shortcut: "⇧⌘I", keywords: ["manifest", "request", "preview", "kv", "cache", "prefix", "tokens", "bytes"]) { [weak self] in self?.showRequestPreview() }, at: 2)
            actions.insert(WorkspaceAction(title: "Inspect Turn Receipts & Honest Cost Ledger", shortcut: "", keywords: ["receipts", "ledger", "cost", "billing", "tokens", "audit"]) { [weak self] in self?.showTurnReceipts() }, at: 3)
            if !hasWork {
                actions.insert(WorkspaceAction(title: "Undo Last Turn File Mutations", shortcut: "⌥⌘Z", keywords: ["undo", "revert", "rollback", "cas", "turn"]) { [weak self] in self?.undoLastTurn() }, at: 4)
            }
        }
        if client.isReady && (swarmDeferEnabled || swarmDeferred) {
            actions.insert(WorkspaceAction(title: swarmDeferred ? "Resume Agent Work" : "Defer Agent Work", shortcut: "", keywords: ["swarm", "pause", "resume", "safe boundary"]) { [weak self] in self?.toggleAgentDefer() }, at: 1)
        }
        if client.isReady {
            actions.insert(WorkspaceAction(title: "Command Recovery…", shortcut: "", keywords: ["terminal", "command", "reconcile", "uncertain", "receipt"]) { [weak self] in self?.showCommandRecovery() }, at: 1)
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
        if canShowContext { actions.insert(WorkspaceAction(title: "Conversation Context", shortcut: "⌘9", keywords: ["history", "summary", "retained", "memory", "inspect", "prompt", "tokens", "budget", "tools", "routing"]) { [weak self] in self?.showContext() }, at: 1) }
        if canCheckCode { actions.insert(WorkspaceAction(title: "Check Code", shortcut: "⇧⌘K", keywords: ["spider", "joyzoning", "rust", "swift", "syntax", "imports", "cycles", "diagnostics"]) { [weak self] in self?.showCodeCheck() }, at: 1) }
        if canShowSavedState { actions.insert(WorkspaceAction(title: "Saved State", shortcut: "", keywords: ["history", "storage", "integrity", "recovery", "runtime"]) { [weak self] in self?.showSavedState() }, at: 1) }
        if canSwitchConversation { actions.insert(WorkspaceAction(title: "New Conversation", shortcut: "⌘N") { [weak self] in self?.newConversation() }, at: 1) }
        if hasWork { actions.insert(WorkspaceAction(title: "Stop All Work", shortcut: "⌘.") { [weak self] in self?.stop() }, at: 1) }
        if canReviewApproval { actions.insert(WorkspaceAction(title: "Review Request…", shortcut: "⌘5") { [weak self] in self?.showApprovalReview() }, at: 1) }
        if canDecideApproval {
            actions.insert(WorkspaceAction(title: "Allow Operation Once", shortcut: "⇧⌘A", keywords: ["approval", "permission", "allow", "run"]) { [weak self] in self?.allowOnce() }, at: 1)
            actions.insert(WorkspaceAction(title: "Allow Operation for Session", shortcut: "", keywords: ["approval", "session", "remember", "allow"]) { [weak self] in self?.allowForSession() }, at: 2)
            actions.insert(WorkspaceAction(title: "Deny Operation", shortcut: "⇧⌘D", keywords: ["approval", "reject", "cancel"]) { [weak self] in self?.deny() }, at: 3)
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
        let modeRequiresFileSearch = (mode == .files)
        guard (!modeRequiresFileSearch || canOpenFile), !closing, let window, window.attachedSheet == nil else { return }
        quickOpen?.dismiss(restoreFocus: false)
        let picker = QuickOpenWindow(folder: folder, search: { [weak self] query, refresh, completion in
            guard let self, self.canOpenFile else {
                completion(.success(["entries": [], "matched": 0, "more": false, "scanLimited": false, "scope": "workspace"]))
                return
            }
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
}

