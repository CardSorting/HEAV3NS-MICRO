// [LAYER: UI]
import AppKit
import Foundation

extension WorkspaceWindow {
    func adapt() {
        let previewOpen = !centerPreview.isHidden
        work.isHidden = previewOpen
        for view in chatChromeViews { view.isHidden = previewOpen }
        let narrow = (window?.frame.width ?? 1280) < 1000
        let short = (window?.frame.height ?? 800) < 640
        navigation.previewsSelection = !short
        leftSidebar.isHidden = previewOpen || (narrow ? !compactSidebarRevealed : !sidebarPreferred)
        let showNav = navigatorTabs.selectedSegment != 0 || navigation.destination == .sessions
        conversations.isHidden = leftSidebar.isHidden || showNav
        navigation.isHidden = leftSidebar.isHidden || !showNav
        surfaces.isHidden = previewOpen || !narrow
        contextPanel.isHidden = previewOpen || !showingInspector || (narrow && (surfaces.selectedSegment == 0 || compactSidebarRevealed))
        contextSeparator.isHidden = contextPanel.isHidden
        let isPreviewActive = contextTabs.selectedSegment == 3 && projectRun.isInAppPreviewVisible
        let frameWidth = window?.frame.width ?? 1280.0
        let maxContextWidth = max(300.0, frameWidth - (leftSidebar.isHidden ? 0.0 : 250.0) - 340.0)
        let defaultContextWidth = narrow ? 300.0 : min(400.0, max(320.0, frameWidth * 0.27))
        let targetWidth: CGFloat
        if isContextMaximized {
            targetWidth = min(maxContextWidth, max(480.0, frameWidth - (narrow ? 20.0 : 280.0)))
        } else if isPreviewActive {
            targetWidth = min(maxContextWidth, max(480.0, min(800.0, frameWidth * 0.52)))
        } else {
            targetWidth = min(maxContextWidth, defaultContextWidth)
        }
        contextWidth?.constant = targetWidth
        walkthrough.isHidden = contextTabs.selectedSegment != 0
        walkthrough.setCompact(short)
        inspector.isHidden = ![1, 2].contains(contextTabs.selectedSegment)
        projectRunScroll.isHidden = contextTabs.selectedSegment != 3
        projectRun.setVisible(!contextPanel.isHidden && !projectRunScroll.isHidden)
        transcriptScroll.isHidden = false
        updateWalkthroughVisibility()
        if transcriptScroll.isHidden || runs.isEmpty { jumpButton.isHidden = true }
        transcript.noteHeightOfRows(withIndexesChanged: IndexSet(integersIn: 0..<rows.count))
    }

    func loadConversations(next: Bool = false) {
        guard client.isReady, client.capabilities.contains("conversations") else { return }
        conversationGeneration += 1; let generation = conversationGeneration
        let offset = next ? conversationOffset + 100 : 0
        let source = client
        source.request("conversation.list", params: ["offset": offset]) { [weak self, weak source] result in
            guard let self, let source, self.client === source, !self.closing, generation == self.conversationGeneration else { return }
            switch result {
            case .success(let data):
                let entries = (data["conversations"] as? [[String: Any]] ?? []).compactMap { value -> ConversationSummary? in
                    guard let id = value["id"] as? String else { return nil }
                    let date = Date(timeIntervalSince1970: (value["updatedAt"] as? Double ?? 0) / 1000).formatted(date: .abbreviated, time: .omitted)
                    return ConversationSummary(id: id, title: value["title"] as? String ?? "Conversation", detail: date)
                }
                self.conversationList = entries; self.conversationOffset = offset; self.moreConversations = data["more"] as? Bool == true
                self.renderConversations()
            case .failure(let error): self.conversations.show(self.conversationList, selected: self.currentConversation, canSwitch: self.canSwitchConversation, more: false, message: error.localizedDescription)
            }
        }
    }

    func renderConversations() {
        conversations.supportsDeletion = client.capabilities.contains("conversation-delete-v1")
        conversations.show(conversationList, selected: currentConversation, canSwitch: canSwitchConversation, more: moreConversations, earlierPage: conversationOffset > 0)
        chatTitle.stringValue = conversationList.first(where: { $0.id == currentConversation })?.title ?? runs.first?["text"] as? String ?? "New conversation"
    }

    @objc func newConversation() { switchConversation(nil) }

    func selectConversation(_ id: String) { switchConversation(id) }

    func confirmDeleteConversation(_ id: String) {
        guard canSwitchConversation, client.capabilities.contains("conversation-delete-v1"),
              let item = conversationList.first(where: { $0.id == id }), let window, window.attachedSheet == nil else { return }
        let alert = NSAlert()
        alert.messageText = "Delete conversation?"
        alert.informativeText = "“\(item.title)” will be removed from the sidebar and its draft discarded. Project files and saved checkpoints are kept.\n\n" + (id == currentConversation ? "A new conversation will open." : "Your current conversation will stay open.")
        alert.addButton(withTitle: "Cancel")
        alert.addButton(withTitle: "Delete Conversation").hasDestructiveAction = true
        alert.beginSheetModal(for: window) { [weak self] response in
            guard response == .alertSecondButtonReturn else { return }
            self?.switchConversation(id, deleting: true)
        }
    }

    func switchConversation(_ id: String?, deleting: Bool = false) {
        guard canSwitchConversation, saveDraft() else { return }
        switchingConversation = true; expectedConversation = nil; sessionContextID = UUID()
        navigationGeneration += 1; inspectionGeneration += 1; conversationGeneration += 1; conversationSwitchGeneration += 1
        walkthroughLoadGeneration += 1; quickOpen?.dismiss()
        updateStatus()
        let switchGen = conversationSwitchGeneration
        let source = client
        source.request(deleting ? "conversation.delete" : (id == nil ? "conversation.create" : "conversation.select"), params: id.map { ["id": $0] } ?? [:]) { [weak self, weak source] result in
            guard let self, let source, self.client === source, !self.closing, self.conversationSwitchGeneration == switchGen else { return }
            switch result {
            case .success(let data):
                if deleting, let id {
                    self.draftByConversation.removeValue(forKey: id)
                    if self.currentConversation == id { self.composer.string = "" }
                    self.savedDraftText = "\u{0}"
                }
                self.expectedConversation = data["conversation"] as? String
                source.requestSnapshot(afterPending: true) { [weak self, weak source] in
                    guard let self, let source, self.client === source, !self.closing, self.conversationSwitchGeneration == switchGen else { return }
                    guard self.currentConversation == self.expectedConversation else {
                        self.switchingConversation = false; self.expectedConversation = nil; self.updateStatus()
                        self.showNotice("The conversation could not be confirmed. Reconnect before sending.")
                        self.client.closeInput(); return
                    }
                    if deleting, let id {
                        self.draftByConversation.removeValue(forKey: id)
                        self.savedDraftText = "\u{0}"
                        if self.saveDraft() { self.showNotice("Conversation deleted. Project files and checkpoints are kept.") }
                    }
                    self.switchingConversation = false; self.expectedConversation = nil
                    self.loadConversations(); self.refreshNavigation(); self.updateStatus()
                    if self.window?.isKeyWindow == true { self.focusComposer() }
                }
            case .failure(let error):
                self.switchingConversation = false; self.expectedConversation = nil
                self.showNotice(error.localizedDescription); self.renderConversations()
                self.refreshNavigation(); self.walkthroughLoaded = nil; self.updateWalkthrough()
            }
            self.updateStatus()
        }
    }

    func adoptConversation(_ id: String) {
        guard id != currentConversation else { return }
        let hadConversation = currentConversation != nil
        if let previous = currentConversation {
            draftByConversation[previous] = composer.string
        } else if !composer.string.isEmpty, (draftByConversation[id] ?? "").isEmpty {
            draftByConversation[id] = composer.string
        }
        composer.string = draftByConversation[id] ?? ""
        currentConversation = id; sessionContextID = UUID()
        runs = []; active = nil; expandedActions.removeAll()
        resetConversationInspection()
        if hadConversation { attachmentBar.clear() }
        receiptBar.isHidden = true
        walkthrough.reset(); walkthroughLoaded = nil; walkthroughLoadGeneration += 1
        savedDraftText = "\u{0}"
        saveDraft()
    }

    func resetConversationInspection() {
        saveViewport()
        viewportSaveTimer?.invalidate(); viewportSaveTimer = nil
        currentViewport = nil; viewportBranch = nil; viewportBranchGeneration += 1
        binaryDocumentLoader?.cancel(); binaryDocumentLoader = nil
        documentView.isHidden = true; inspectionScroll.isHidden = false
        layoutTimer?.invalidate(); layoutTimer = nil; needsRebuild = false
        navigationGeneration += 1; inspectionGeneration += 1
        workspaceRefreshTimer?.invalidate(); workspaceRefreshTimer = nil
        inspectionHistory = InspectionHistory(); reloadInspection = nil; nextChunk = nil; previousChunk = nil
        inspectionBack.isEnabled = false; inspectionForward.isEnabled = false
        toolbarHistory.setEnabled(false, forSegment: 0)
        toolbarHistory.setEnabled(false, forSegment: 1)
        updateJumpBar()
        inspectedFile = nil; inspectedDiff = nil; declarations = []
        inspection.string = "Choose a file, change, or recorded instruction to inspect."
        inspectorTitle.stringValue = "Inspect"; pageLabel.stringValue = "Read only"
        evidenceLinks = []; contextLinks = []; showingContextSummary = false; relatedEvidence.isHidden = true
        spiderDetails = nil; checkCode.isHidden = true; prepareCodeReview.isHidden = true
        [breadcrumb, diffScope, revealButton, outline, recordedOutput, moreButton, previousButton].forEach { $0.isHidden = true }
        nextNavigationPage = nil; previousNavigationPage = nil; navigationData = []
        navigationQuery = ""; navigation.resetFilter()
        navigation.show([], location: navigation.destination.title, message: "Opening conversation…")
    }

    @objc func jumpLatest() { if !rows.isEmpty { transcript.scrollRowToVisible(rows.count - 1) }; jumpButton.isHidden = true }

    @objc func toggleNavigation() {
        if (window?.frame.width ?? 1280) < 1000 {
            compactSidebarRevealed.toggle()
            if compactSidebarRevealed { surfaces.selectedSegment = 0 }
        } else { sidebarPreferred.toggle() }
        adapt()
    }

    @objc func showChats() {
        navigatorTabs.selectedSegment = 0
        sidebarPreferred = true
        compactSidebarRevealed = true
        switchNavigatorTab()
    }

    @objc func showFiles() {
        navigatorTabs.selectedSegment = 1
        sidebarPreferred = true
        compactSidebarRevealed = true
        navigate(.files)
    }

    @objc func showChanges() {
        navigatorTabs.selectedSegment = 2
        sidebarPreferred = true
        compactSidebarRevealed = true
        navigate(.changes)
    }

    @objc func showSessions() {
        loadConversations()
        conversations.isHidden = true; navigation.isHidden = false
        sidebarPreferred = true; compactSidebarRevealed = true
        navigation.select(.sessions); adapt()
    }

    @objc func switchNavigatorTab() {
        switch navigatorTabs.selectedSegment {
        case 0:
            if navigation.destination == .sessions { navigation.select(.files) }
            conversations.isHidden = false
            navigation.isHidden = true
        case 1:
            conversations.isHidden = true
            navigation.isHidden = false
            navigation.select(.files)
        case 2:
            conversations.isHidden = true
            navigation.isHidden = false
            navigation.select(.changes)
        default:
            break
        }
        adapt()
    }

    func navigate(_ destination: WorkspaceDestination) {
        navigationGeneration += 1; navigationQuery = ""; navigationOffset = 0; nextNavigationPage = nil; previousNavigationPage = nil; navigationData = []
        showingContextSummary = false
        showingInspector = true; surfaces.selectedSegment = 1
        browsingCompact = true
        if destination == .files {
            navigatorTabs.selectedSegment = 1
            conversations.isHidden = true
            navigation.isHidden = false
            contextTabs.selectedSegment = 2
        } else if destination == .changes {
            navigatorTabs.selectedSegment = 2
            conversations.isHidden = true
            navigation.isHidden = false
            contextTabs.selectedSegment = 1
        } else {
            navigatorTabs.selectedSegment = 1
            conversations.isHidden = true
            navigation.isHidden = false
            contextTabs.selectedSegment = 2
        }
        compactSidebarRevealed = false
        adapt(); refreshNavigation()
    }

    func refreshNavigation() {
        guard !switchingConversation else { return }
        guard client.isReady else { navigation.loading("Connect to LUMI to browse this workspace."); return }
        switch navigation.destination {
        case .files: loadDirectory(directory, offset: navigationOffset)
        case .changes: loadChanges()
        case .sessions: loadSessions()
        }
    }

    func scheduleWorkspaceRefresh() {
        guard workspaceRefreshTimer == nil else { return }
        workspaceRefreshTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: false) { [weak self] _ in
            guard let self else { return }; self.workspaceRefreshTimer = nil
            guard self.client.isReady else { return }
            if !self.navigation.isHiddenOrHasHiddenAncestor, self.navigation.destination != .sessions { self.refreshNavigation() }
            if self.inspectedFile != nil { self.pageLabel.stringValue = "Workspace activity recorded · Reload to update preview" }
            self.checkViewportBranch()
        }
    }

    func loadDirectory(_ path: String, offset: Int = 0) {
        if path != directory { navigationQuery = ""; navigation.resetFilter() }
        directory = path; navigationOffset = offset; navigationGeneration += 1
        nextNavigationPage = nil; previousNavigationPage = nil; navigationData = []
        let generation = navigationGeneration
        let source = client
        navigation.loading("Loading folder…")
        source.request("files.list", params: ["path": path, "offset": offset, "query": navigationQuery]) { [weak self, weak source] result in
            guard let self, let source, self.client === source, !self.closing, self.navigationGeneration == generation, self.navigation.destination == .files else { return }
            switch result {
            case .success(let data):
                self.navigationData = (data["entries"] as? [[String: Any]] ?? []).map { entry in
                    SidebarEntry(id: entry["path"] as? String ?? "", title: entry["name"] as? String ?? "File", detail: entry["symlink"] as? Bool == true ? "Symbolic link" : "", symbol: entry["directory"] as? Bool == true ? "folder" : "doc", value: entry)
                }
                self.nextNavigationPage = data["more"] as? Bool == true ? { [weak self] in self?.loadDirectory(path, offset: data["nextOffset"] as? Int ?? 0) } : nil
                self.previousNavigationPage = offset > 0 ? { [weak self] in self?.loadDirectory(path, offset: max(0, offset - 100)) } : nil
                let count = self.navigationData.count
                let message = data["scanLimited"] as? Bool == true ? "Folder scan limited to 20,000 entries. Open a subfolder to narrow the list." : count == 0 ? (self.navigationQuery.isEmpty ? "This folder is empty." : "No matching files in this folder.") : "\(offset + 1)–\(offset + count) of \(data["matched"] as? Int ?? count)"
                self.navigation.show(self.navigationData, location: path == "." ? self.folder.lastPathComponent : path, message: message, canGoBack: path != ".", more: self.nextNavigationPage != nil, hasPrevious: offset > 0)
            case .failure(let error): self.navigation.show([], location: path, message: error.localizedDescription, canGoBack: path != ".")
            }
        }
    }

    @objc func parentFolder() {
        let parent = (directory as NSString).deletingLastPathComponent
        loadDirectory(parent.isEmpty ? "." : parent)
    }

    func configureSourceControlCallbacks() {
        navigation.onInitGit = { [weak self] in self?.runGitAction("git.init", params: [:], successNotice: "Initialized Git repository.") }
        navigation.onOpenSourceControl = { [weak self] in self?.showSourceControl() }
    }

    @objc func showSourceControl() {
        guard client.isReady else { showNotice("Connect to LUMI to open Source Control."); return }
        if let existing = sourceControlWindow {
            existing.present()
            return
        }
        let win = SourceControlWindow(folder: folder) { [weak self] method, params, completion in
            guard let self else { return }
            self.client.request(method, params: params, completion: completion)
        }
        win.onClose = { [weak self] in self?.sourceControlWindow = nil }
        sourceControlWindow = win
        win.present()
    }

    func runGitAction(_ method: String, params: [String: Any], successNotice: String? = nil, onSuccess: (() -> Void)? = nil) {
        guard client.isReady else { return }
        client.request(method, params: params) { [weak self] result in
            guard let self, !self.closing else { return }
            switch result {
            case .success(let data):
                if data["ok"] as? Bool == false {
                    self.showNotice(data["error"] as? String ?? "Git operation failed.")
                    return
                }
                if let successNotice { self.showNotice(successNotice) }
                onSuccess?()
                self.loadChanges()
            case .failure(let error):
                self.showNotice(error.localizedDescription)
            }
        }
    }

    func loadChanges() {
        navigationGeneration += 1; let generation = navigationGeneration; navigation.loading("Reading Git changes…")
        nextNavigationPage = nil; previousNavigationPage = nil; navigationData = []
        let source = client
        source.request("changes.list") { [weak self, weak source] result in
            guard let self, let source, self.client === source, !self.closing, self.navigationGeneration == generation, self.navigation.destination == .changes else { return }
            switch result {
            case .success(let data):
                let rawEntries = data["entries"] as? [[String: Any]] ?? []
                var grouped = [SidebarEntry]()
                let stagedEntries = rawEntries.filter { $0["staged"] as? Bool == true }
                let unstagedEntries = rawEntries.filter { $0["unstaged"] as? Bool == true || $0["untracked"] as? Bool == true || $0["staged"] as? Bool != true }
                if !stagedEntries.isEmpty {
                    grouped.append(SidebarEntry(id: "__section_staged", title: "Staged Changes", detail: "", symbol: "checkmark.circle", value: ["isSectionHeader": true, "sectionCount": stagedEntries.count]))
                    for entry in stagedEntries {
                        var v = entry; v["section"] = "staged"
                        let st = entry["status"] as? String ?? "modified"
                        v["statusBadge"] = st == "added" ? "A" : st == "deleted" ? "D" : st == "renamed" ? "R" : "M"
                        grouped.append(SidebarEntry(id: entry["path"] as? String ?? "", title: entry["name"] as? String ?? "File", detail: entry["path"] as? String ?? "", symbol: "doc.text", value: v))
                    }
                }
                if !unstagedEntries.isEmpty {
                    grouped.append(SidebarEntry(id: "__section_changes", title: "Changes", detail: "", symbol: "pencil", value: ["isSectionHeader": true, "sectionCount": unstagedEntries.count]))
                    for entry in unstagedEntries {
                        var v = entry; v["section"] = "changes"
                        let st = entry["status"] as? String ?? "modified"
                        v["statusBadge"] = entry["untracked"] as? Bool == true ? "U" : st == "deleted" ? "D" : st == "added" ? "A" : "M"
                        grouped.append(SidebarEntry(id: entry["path"] as? String ?? "", title: entry["name"] as? String ?? "File", detail: entry["path"] as? String ?? "", symbol: "doc.text", value: v))
                    }
                }
                self.navigationData = grouped
                let isGitRepo = data["isGitRepo"] as? Bool ?? true
                let branch = data["branch"] as? String ?? "unversioned"
                self.navigation.setSourceControlState(isGitRepo: isGitRepo, branch: branch, hasChanges: !rawEntries.isEmpty)
                self.window?.subtitle = "\(branch) · \(self.folder.path)"
                self.nextNavigationPage = nil; self.previousNavigationPage = nil; self.renderNavigation()
                if data["limited"] as? Bool == true { self.showNotice("Showing the first 500 changed paths. Use Git in an external terminal to inspect the complete list.") }
            case .failure(let error):
                self.navigation.setSourceControlState(isGitRepo: false, branch: "unversioned", hasChanges: false)
                self.navigation.show([], location: "Source Control", message: "Changes require a Git workspace. \(error.localizedDescription)")
            }
        }
    }

    func loadSessions(cursor: [String: Any]? = nil, trail: [[String: Any]] = []) {
        navigationGeneration += 1; let generation = navigationGeneration; navigation.loading("Loading recorded runs…")
        nextNavigationPage = nil; previousNavigationPage = nil; navigationData = []
        let source = client
        source.request("history.list", params: cursor.map { ["cursor": $0] } ?? [:]) { [weak self, weak source] result in
            guard let self, let source, self.client === source, !self.closing, self.navigationGeneration == generation, self.navigation.destination == .sessions else { return }
            switch result {
            case .success(let data):
                self.navigationData = (data["runs"] as? [[String: Any]] ?? []).reversed().map { run in
                    let date = Date(timeIntervalSince1970: (run["createdAt"] as? Double ?? 0) / 1000).formatted(date: .abbreviated, time: .shortened)
                    return SidebarEntry(id: run["id"] as? String ?? "", title: run["text"] as? String ?? "Run", detail: "\(run["state"] as? String ?? "Unknown") · \(date)", symbol: "text.bubble", value: run)
                }
                self.nextNavigationPage = (data["nextCursor"] as? [String: Any]).map { next in { [weak self] in self?.loadSessions(cursor: next, trail: Array((trail + [cursor ?? [:]]).suffix(128))) } }
                self.previousNavigationPage = trail.last.map { previous in { [weak self] in self?.loadSessions(cursor: previous.isEmpty ? nil : previous, trail: Array(trail.dropLast())) } }
                self.renderNavigation()
            case .failure(let error): self.navigation.show([], location: "Sessions", message: error.localizedDescription)
            }
        }
    }

    func renderNavigation() {
        let filtered = navigationData.filter { navigationQuery.isEmpty || $0.title.localizedCaseInsensitiveContains(navigationQuery) || $0.detail.localizedCaseInsensitiveContains(navigationQuery) }
        let empty = navigation.destination == .changes ? "No changes in the working tree or index." : "No recorded runs on this page."
        navigation.show(filtered, location: navigation.destination.title, message: filtered.isEmpty ? (navigationQuery.isEmpty ? empty : "No matches on this page.") : navigation.destination == .changes ? "Includes pre-existing and external edits." : "Recorded runs · no automatic resume", more: nextNavigationPage != nil, hasPrevious: previousNavigationPage != nil)
    }

    func openEntry(_ entry: SidebarEntry) {
        switch navigation.destination {
        case .files:
            if entry.value["directory"] as? Bool == true { loadDirectory(entry.id) } else { readFile(entry.id) }
        case .changes:
            if entry.value["untracked"] as? Bool == true { readFile(entry.id) }
            else { diffScope.selectedSegment = entry.value["unstaged"] as? Bool == true ? 0 : 1; readDiff(entry.id) }
        case .sessions: inspect(.run(entry.id))
        }
    }

    func readFile(_ rawPath: String) {
        let (path, line, col) = QuickOpenWindow.parseFileLineCol(rawPath)
        if inspectedFile == path, let line {
            goToLine(line, column: col ?? 1)
            return
        }
        if let line { self.pendingLineJump = (line, col ?? 1) }
        inspect(.file(path))
    }

    func readDiff(_ path: String) { inspect(.diff(path, staged: diffScope.selectedSegment == 1)) }

    private final class SidebarMenuActionTarget: NSObject {
        private var block: (() -> Void)?
        init(_ block: @escaping () -> Void) { self.block = block }
        @objc func invoke() { block?() }
        func dispose() { block = nil }
    }

    private func safeWorkspaceURL(for relativePath: String) -> URL? {
        guard WalkthroughActivity.safeRelativePath(relativePath) else { return nil }
        let rootPath = folder.standardizedFileURL.path
        let candidate = folder.appendingPathComponent(relativePath).standardizedFileURL
        guard candidate.path.hasPrefix(rootPath + "/") else { return nil }
        return candidate
    }

    private func addMenuItem(_ menu: NSMenu, title: String, symbol: String? = nil, enabled: Bool = true, action: @escaping () -> Void) {
        let item = NSMenuItem(title: title, action: #selector(SidebarMenuActionTarget.invoke), keyEquivalent: "")
        let target = SidebarMenuActionTarget(action)
        item.target = target
        item.isEnabled = enabled
        if let symbol, let img = NSImage(systemSymbolName: symbol, accessibilityDescription: nil) {
            item.image = img
        }
        objc_setAssociatedObject(item, Unmanaged.passUnretained(item).toOpaque(), target, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        menu.addItem(item)
    }

    private func popUpSidebarMenu(_ menu: NSMenu, from anchor: NSView) {
        menu.popUp(positioning: nil, at: NSPoint(x: 0, y: anchor.bounds.height + 2), in: anchor)
    }

    func buildSidebarWorkspaceMenu() -> NSMenu {
        let menu = NSMenu(title: "Workspace Options")
        addMenuItem(menu, title: "Chats (⇧⌘C)", symbol: "bubble.left.and.bubble.right") { [weak self] in self?.showChats() }
        addMenuItem(menu, title: "Files (⌘1)", symbol: "folder") { [weak self] in self?.showFiles() }
        addMenuItem(menu, title: "Changes / Diffs (⌘2)", symbol: "arrow.triangle.branch") { [weak self] in self?.showChanges() }
        addMenuItem(menu, title: "Open Source Control Window… (⇧⌘G)", symbol: "arrow.triangle.pull", enabled: client.isReady) { [weak self] in self?.showSourceControl() }
        addMenuItem(menu, title: "Recorded Runs (⌘3)", symbol: "clock") { [weak self] in self?.showSessions() }
        menu.addItem(.separator())
        addMenuItem(menu, title: "Walkthrough (⌘6)", symbol: "list.bullet.rectangle") { [weak self] in self?.showWalkthrough() }
        addMenuItem(menu, title: "Up Next Queue (⌘4)", symbol: "tray.full") { [weak self] in self?.showQueue() }
        addMenuItem(menu, title: "Checkpoints…", symbol: "clock.arrow.circlepath", enabled: client.isReady) { [weak self] in self?.showCheckpoints() }
        if let conv = currentConversation {
            addMenuItem(menu, title: "Conversation Context", symbol: "doc.text.magnifyingglass", enabled: canShowContext) { [weak self] in self?.inspect(.context(conv)) }
            addMenuItem(menu, title: "Saved State", symbol: "internaldrive", enabled: canShowSavedState) { [weak self] in self?.inspect(.savedState(conv)) }
        }
        addMenuItem(menu, title: "Conversation Memory…", symbol: "brain", enabled: client.isReady) { [weak self] in self?.showMemory() }
        addMenuItem(menu, title: "Command Recovery…", symbol: "terminal", enabled: client.isReady) { [weak self] in self?.showCommandRecovery() }
        addMenuItem(menu, title: "Agent Swarm Settings…", symbol: "person.3", enabled: client.isReady) { [weak self] in self?.showAgentSwarmSettings() }
        addMenuItem(menu, title: "Turn Receipts & Cost Ledger", symbol: "receipt", enabled: client.isReady) { [weak self] in self?.showTurnReceipts() }
        addMenuItem(menu, title: "Outbound Request Preview (⇧⌘I)", symbol: "network", enabled: client.isReady) { [weak self] in self?.showRequestPreview() }
        addMenuItem(menu, title: "Sovereign Swarm Radar… (⌥⌘R)", symbol: "dot.radiowaves.left.and.right") { SwarmRadarWindow.shared.present() }
        menu.addItem(.separator())
        addMenuItem(menu, title: "New Conversation", symbol: "plus.bubble", enabled: client.isReady && !hasWork) { [weak self] in self?.newConversation() }
        addMenuItem(menu, title: "Quick Open File… (⌘P)", symbol: "magnifyingglass", enabled: canOpenFile) { [weak self] in self?.showQuickOpen() }
        addMenuItem(menu, title: "Command Palette… (⌘K)", symbol: "command") { [weak self] in self?.showCommandPalette() }
        addMenuItem(menu, title: "Choose Model & Provider… (⇧⌘M)", symbol: "cpu", enabled: client.capabilities.contains("model-selection-v1")) { [weak self] in self?.showModels() }
        addMenuItem(menu, title: "Cycle Execution Posture (\(activePosture.stepNext().label))", symbol: "slider.horizontal.3") { [weak self] in self?.stepPosture() }
        if client.capabilities.contains("agent-profiles-v1") {
            let profileItem = NSMenuItem(title: "Agent Profile (HEAV3NS-JOY)", action: nil, keyEquivalent: "")
            if let img = NSImage(systemSymbolName: "person.badge.shield.checkmark", accessibilityDescription: nil) { profileItem.image = img }
            let sub = NSMenu(title: "Agent Profile")
            for (pid, label) in [("joyzoning-architect", "Joyzoning Architect (Default)"), ("strict-ci", "Strict CI Guardian"), ("fast-patch", "Fast Patch"), ("research-readonly", "Research Read-Only")] {
                addMenuItem(sub, title: label, symbol: pid == activeProfileID ? "checkmark.circle.fill" : "circle", enabled: client.isReady) { [weak self] in self?.switchAgentProfile(pid) }
                sub.items.last?.state = (pid == activeProfileID) ? .on : .off
            }
            profileItem.submenu = sub
            menu.addItem(profileItem)
        }
        if client.capabilities.contains("joy-ci-gate-v1") {
            addMenuItem(menu, title: "Run HEAV3NS-JOY CI Gate", symbol: "checkmark.shield", enabled: client.isReady) { [weak self] in self?.runJoyCiGate() }
        }
        addMenuItem(menu, title: "Undo Last Turn (⌥⌘Z)", symbol: "arrow.uturn.backward", enabled: client.isReady && !hasWork) { [weak self] in self?.undoLastTurn() }
        addMenuItem(menu, title: "Refresh Current View", symbol: "arrow.clockwise") { [weak self] in self?.refreshNavigation() }
        menu.addItem(.separator())
        addMenuItem(menu, title: "Open Workspace… (⌘O)", symbol: "folder.badge.plus") { NSApp.sendAction(#selector(ApplicationDelegate.openFolder), to: NSApp.delegate, from: nil) }
        addMenuItem(menu, title: "New Project… (⇧⌘N)", symbol: "plus.square.on.square") { NSApp.sendAction(#selector(ApplicationDelegate.createProject), to: NSApp.delegate, from: nil) }
        addMenuItem(menu, title: "Project Library… (⌘0)", symbol: "books.vertical") { [weak self] in self?.onProjects?() }
        addMenuItem(menu, title: "Account…", symbol: "person.crop.circle") { [weak self] in self?.onAccount?() }
        addMenuItem(menu, title: "Maximize / Restore Context", symbol: "arrow.up.left.and.arrow.down.right") { [weak self] in self?.toggleMaximizeContext() }
        addMenuItem(menu, title: "Hide Sidebar (⌘B)", symbol: "sidebar.left") { [weak self] in self?.toggleNavigation() }
        return menu
    }

    func presentSidebarWorkspaceMenu(from anchor: NSView) {
        popUpSidebarMenu(buildSidebarWorkspaceMenu(), from: anchor)
    }

    func buildNavigationEntryMenu(_ entry: SidebarEntry) -> NSMenu {
        let menu = NSMenu(title: entry.title)
        switch navigation.destination {
        case .files:
            let isDir = entry.value["directory"] as? Bool == true
            if isDir {
                addMenuItem(menu, title: "Open Folder", symbol: "folder") { [weak self] in self?.loadDirectory(entry.id) }
            } else {
                addMenuItem(menu, title: "Open File", symbol: "doc.text") { [weak self] in self?.readFile(entry.id) }
                addMenuItem(menu, title: "View Working Tree Diff", symbol: "arrow.left.arrow.right.square") { [weak self] in self?.inspect(.diff(entry.id, staged: false)) }
                addMenuItem(menu, title: "View Staged Diff", symbol: "tray.and.arrow.down") { [weak self] in self?.inspect(.diff(entry.id, staged: true)) }
                addMenuItem(menu, title: "Run Code Check (Spider)", symbol: "checkmark.shield", enabled: self.client.capabilities.contains("spider-check-v1")) { [weak self] in self?.inspect(.spider(entry.id)) }
            }
            menu.addItem(.separator())
            addMenuItem(menu, title: "Copy Relative Path", symbol: "doc.on.doc") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(entry.id, forType: .string)
            }
            addMenuItem(menu, title: "Reveal in Finder", symbol: "finder", enabled: safeWorkspaceURL(for: entry.id) != nil) { [weak self] in
                guard let url = self?.safeWorkspaceURL(for: entry.id) else { return }
                NSWorkspace.shared.activateFileViewerSelecting([url])
            }
        case .changes:
            let isStaged = entry.value["staged"] as? Bool == true
            let isUnstaged = entry.value["unstaged"] as? Bool == true
            let isUntracked = entry.value["untracked"] as? Bool == true
            if isUnstaged || isUntracked {
                addMenuItem(menu, title: "Stage File", symbol: "plus.circle") { [weak self] in self?.runGitAction("git.stage", params: ["path": entry.id]) }
            }
            if isStaged {
                addMenuItem(menu, title: "Unstage File", symbol: "minus.circle") { [weak self] in self?.runGitAction("git.unstage", params: ["path": entry.id]) }
            }
            if isUnstaged && !isUntracked {
                addMenuItem(menu, title: "Discard Working Tree Changes", symbol: "arrow.uturn.backward.circle") { [weak self] in
                    guard let self, let window = self.window else { return }
                    let alert = NSAlert()
                    alert.messageText = "Discard changes to \(entry.title)?"
                    alert.informativeText = "This will revert unstaged edits to \(entry.id) back to HEAD."
                    alert.addButton(withTitle: "Cancel")
                    alert.addButton(withTitle: "Discard").hasDestructiveAction = true
                    alert.beginSheetModal(for: window) { response in
                        guard response == .alertSecondButtonReturn else { return }
                        self.runGitAction("git.discard", params: ["path": entry.id], successNotice: "Discarded changes to \(entry.title).")
                    }
                }
            }
            menu.addItem(.separator())
            addMenuItem(menu, title: "View Working Tree Diff", symbol: "arrow.left.arrow.right.square") { [weak self] in self?.inspect(.diff(entry.id, staged: false)) }
            addMenuItem(menu, title: "View Staged Diff", symbol: "tray.and.arrow.down") { [weak self] in self?.inspect(.diff(entry.id, staged: true)) }
            addMenuItem(menu, title: "Open File", symbol: "doc.text") { [weak self] in self?.readFile(entry.id) }
            addMenuItem(menu, title: "Run Code Check (Spider)", symbol: "checkmark.shield", enabled: self.client.capabilities.contains("spider-check-v1")) { [weak self] in self?.inspect(.spider(entry.id)) }
            menu.addItem(.separator())
            addMenuItem(menu, title: "Copy Relative Path", symbol: "doc.on.doc") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(entry.id, forType: .string)
            }
            addMenuItem(menu, title: "Reveal in Finder", symbol: "finder", enabled: safeWorkspaceURL(for: entry.id) != nil) { [weak self] in
                guard let url = self?.safeWorkspaceURL(for: entry.id) else { return }
                NSWorkspace.shared.activateFileViewerSelecting([url])
            }
        case .sessions:
            addMenuItem(menu, title: "Inspect Run Details", symbol: "doc.text.magnifyingglass") { [weak self] in self?.inspect(.run(entry.id)) }
            addMenuItem(menu, title: "Open in Walkthrough", symbol: "list.bullet.rectangle") { [weak self] in
                self?.showWalkthrough()
                self?.walkthrough.selectRecordedRun(entry.id)
            }
            menu.addItem(.separator())
            addMenuItem(menu, title: "Copy Run ID", symbol: "doc.on.doc") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(entry.id, forType: .string)
            }
        }
        return menu
    }

    func presentNavigationEntryMenu(_ entry: SidebarEntry, from anchor: NSView) {
        popUpSidebarMenu(buildNavigationEntryMenu(entry), from: anchor)
    }

    func buildConversationMenu(_ item: ConversationSummary) -> NSMenu {
        let menu = NSMenu(title: item.title)
        addMenuItem(menu, title: "Open Conversation", symbol: "bubble.left", enabled: client.isReady && !hasWork) { [weak self] in self?.selectConversation(item.id) }
        addMenuItem(menu, title: "Copy Conversation ID", symbol: "doc.on.doc") {
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(item.id, forType: .string)
        }
        if conversations.supportsDeletion {
            menu.addItem(.separator())
            addMenuItem(menu, title: "Delete Conversation…", symbol: "trash", enabled: client.isReady && !hasWork) { [weak self] in self?.confirmDeleteConversation(item.id) }
        }
        return menu
    }

    func presentConversationMenu(_ item: ConversationSummary, from anchor: NSView) {
        popUpSidebarMenu(buildConversationMenu(item), from: anchor)
    }

    func switchAgentProfile(_ profileId: String) {
        guard client.isReady else { return }
        let source = client
        source.request("profile.set", params: ["id": profileId]) { [weak self, weak source] result in
            guard let self, let source, self.client === source, !self.closing else { return }
            switch result {
            case .success(let data):
                let prof = data["activeProfile"] as? [String: Any]
                let resolvedId = prof?["id"] as? String ?? profileId
                let name = prof?["name"] as? String ?? resolvedId
                self.activeProfileID = resolvedId
                if let remotePosture = data["posture"] as? String, let stop = ExecutionPostureStop(rawValue: remotePosture) {
                    self.applyPosture(stop, syncRemote: false)
                }
                self.showNotice("Active profile switched to \(name).")
            case .failure(let err):
                self.showNotice(err.localizedDescription)
            }
        }
    }

    func runJoyCiGate() {
        guard client.isReady else { return }
        let source = client
        showNotice("Running HEAV3NS-JOY strict CI gate…")
        source.request("ci_gate.run", params: [:]) { [weak self, weak source] result in
            guard let self, let source, self.client === source, !self.closing else { return }
            switch result {
            case .success(let data):
                let passed = data["passed"] as? Bool == true
                let theme = data["theme"] as? String ?? "strict"
                let phase = data["phase"] as? String ?? "ci"
                let blocking = data["blockingCount"] as? Int ?? 0
                let advisory = data["advisoryCount"] as? Int ?? 0
                let ms = data["durationMs"] as? Int ?? 0
                let summary = data["summary"] as? String ?? ""
                self.showNotice("HEAV3NS-JOY CI Gate \(passed ? "PASSED" : "FAILED") · \(blocking) blocking · \(advisory) advisory (\(ms)ms)")
                if !passed || !summary.isEmpty {
                    self.prepareInspection(title: "HEAV3NS-JOY CI Gate (\(passed ? "PASS" : "FAIL"))", file: nil, contextSummary: true)
                    self.pageLabel.stringValue = "theme=\(theme) · phase=\(phase) · \(blocking) blocking · \(advisory) advisory · \(ms)ms"
                    self.inspection.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
                    self.inspection.string = summary.isEmpty ? "CI gate passed with 0 blocking violations." : summary
                    self.inspection.scrollToBeginningOfDocument(nil)
                    self.showingInspector = true; self.adapt()
                }
            case .failure(let err):
                self.showNotice(err.localizedDescription)
            }
        }
    }
}

