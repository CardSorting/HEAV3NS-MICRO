// [LAYER: UI]
import AppKit
import Foundation

extension WorkspaceWindow {
    func loadConversations(next: Bool = false) {
        guard client.isReady, client.capabilities.contains("conversations") else { return }
        conversationGeneration += 1; let generation = conversationGeneration
        let offset = next ? conversationOffset + 100 : 0
        client.request("conversation.list", params: ["offset": offset]) { [weak self] result in
            guard let self, generation == self.conversationGeneration else { return }
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
        switchingConversation = true; expectedConversation = nil; podContextID = UUID()
        navigationGeneration += 1; inspectionGeneration += 1; conversationGeneration += 1
        walkthroughLoadGeneration += 1; quickOpen?.dismiss()
        updateStatus()
        client.request(deleting ? "conversation.delete" : (id == nil ? "conversation.create" : "conversation.select"), params: id.map { ["id": $0] } ?? [:]) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let data):
                if deleting, let id {
                    self.draftByConversation.removeValue(forKey: id)
                    if self.currentConversation == id { self.composer.string = "" }
                    self.savedDraftText = "\u{0}"
                }
                self.expectedConversation = data["conversation"] as? String
                self.client.requestSnapshot(afterPending: true) { [weak self] in
                    guard let self else { return }
                    guard self.currentConversation == self.expectedConversation else {
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
        if let previous = currentConversation {
            draftByConversation[previous] = composer.string
            composer.string = draftByConversation[id] ?? ""
        }
        currentConversation = id; podContextID = UUID()
        runs = []; active = nil; expandedActions.removeAll()
        resetConversationInspection()
        walkthrough.reset(); walkthroughLoaded = nil; walkthroughLoadGeneration += 1
        savedDraftText = "\u{0}"
        saveDraft()
    }

    func resetConversationInspection() {
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

    @objc func showSessions() { loadConversations(); navigation.select(.sessions) }

    @objc func switchNavigatorTab() {
        switch navigatorTabs.selectedSegment {
        case 0:
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
        navigation.loading("Loading folder…")
        client.request("files.list", params: ["path": path, "offset": offset, "query": navigationQuery]) { [weak self] result in
            guard let self, self.navigationGeneration == generation, self.navigation.destination == .files else { return }
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

    func loadChanges() {
        navigationGeneration += 1; let generation = navigationGeneration; navigation.loading("Reading Git changes…")
        nextNavigationPage = nil; previousNavigationPage = nil; navigationData = []
        client.request("changes.list") { [weak self] result in
            guard let self, self.navigationGeneration == generation, self.navigation.destination == .changes else { return }
            switch result {
            case .success(let data):
                self.navigationData = (data["entries"] as? [[String: Any]] ?? []).map { entry in
                    let status = entry["untracked"] as? Bool == true ? "Untracked" : entry["staged"] as? Bool == true ? (entry["unstaged"] as? Bool == true ? "Staged + working tree" : "Staged") : "Working tree"
                    return SidebarEntry(id: entry["path"] as? String ?? "", title: entry["name"] as? String ?? "File", detail: "\(entry["status"] as? String ?? "") · \(status)", symbol: "doc.text", value: entry)
                }
                if let branch = data["branch"] as? String { self.window?.subtitle = "\(branch) · \(self.folder.path)" }
                self.nextNavigationPage = nil; self.previousNavigationPage = nil; self.renderNavigation()
                if data["limited"] as? Bool == true { self.showNotice("Showing the first 500 changed paths. Use Git in an external terminal to inspect the complete list.") }
            case .failure(let error): self.navigation.show([], location: "Changes", message: "Changes require a Git workspace. \(error.localizedDescription)")
            }
        }
    }

    func loadSessions(cursor: [String: Any]? = nil, trail: [[String: Any]] = []) {
        navigationGeneration += 1; let generation = navigationGeneration; navigation.loading("Loading recorded runs…")
        nextNavigationPage = nil; previousNavigationPage = nil; navigationData = []
        client.request("history.list", params: cursor.map { ["cursor": $0] } ?? [:]) { [weak self] result in
            guard let self, self.navigationGeneration == generation, self.navigation.destination == .sessions else { return }
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

    func readFile(_ path: String) { inspect(.file(path)) }

    func readDiff(_ path: String) { inspect(.diff(path, staged: diffScope.selectedSegment == 1)) }
}

