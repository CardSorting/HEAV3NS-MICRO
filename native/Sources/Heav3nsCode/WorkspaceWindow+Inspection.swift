// [LAYER: UI]
import AppKit
import Foundation

extension WorkspaceWindow {
    func inspect(_ destination: InspectionDestination, addToHistory: Bool = true) {
        guard !switchingConversation else { return }
        saveViewport()
        currentViewport = nil; viewportBranch = nil; viewportBranchGeneration += 1
        if case .diff = destination { contextTabs.selectedSegment = 1 }
        else { contextTabs.selectedSegment = 2 }
        if addToHistory { inspectionHistory.visit(destination) }
        inspectionBack.isEnabled = inspectionHistory.canGoBack; inspectionForward.isEnabled = inspectionHistory.canGoForward
        toolbarHistory.setEnabled(inspectionHistory.canGoBack, forSegment: 0)
        toolbarHistory.setEnabled(inspectionHistory.canGoForward, forSegment: 1)
        updateJumpBar()
        reloadInspection = { [weak self] in self?.inspect(destination, addToHistory: false) }
        switch destination {
        case .savedState(let conversation):
            prepareInspection(title: "Saved State", file: nil, contextSummary: true)
            guard canShowSavedState, currentConversation == conversation else {
                inspectionFailed(BridgeError.message("Saved state is unavailable. Reconnect to a supported runtime."), initial: true); return
            }
            let generation = inspectionGeneration
            client.request("state.inspect", params: ["conversation": conversation]) { [weak self] result in
                guard let self, self.inspectionGeneration == generation, self.currentConversation == conversation else { return }
                switch result {
                case .success(let data):
                    guard let details = SavedStateDetails(data, conversation: conversation) else {
                        self.inspectionFailed(BridgeError.message("Saved state could not be read. Use Reload to try again."), initial: true); return
                    }
                    self.savedStateDetails = details
                    self.inspection.textStorage?.setAttributedString(details.attributedText); self.inspection.scrollToBeginningOfDocument(nil)
                    self.pageLabel.stringValue = details.observed; self.pageLabel.toolTip = details.observed
                    self.relatedEvidence.removeAllItems(); self.relatedEvidence.addItems(withTitles: ["Open Instruction…"] + details.report.runs.map { String($0.title.prefix(60)) })
                    self.relatedEvidence.setAccessibilityLabel("Open a saved instruction")
                    self.relatedEvidence.toolTip = "Review the selected instruction’s saved result and related evidence"
                    self.relatedEvidence.isHidden = details.report.runs.isEmpty
                case .failure(let error): self.inspectionFailed(error, initial: true)
                }
            }
        case .spider(let file):
            prepareInspection(title: "Code Check", file: nil, contextSummary: true)
            inspection.font = .systemFont(ofSize: 14)
            guard client.isReady, client.capabilities.contains("spider-check-v1") else {
                inspectionFailed(BridgeError.message("Code checks are unavailable. Reconnect to a supported runtime."), initial: true); return
            }
            let generation = inspectionGeneration
            client.request("spider.check", params: ["paths": [file]]) { [weak self] result in
                guard let self, self.inspectionGeneration == generation else { return }
                switch result {
                case .success(let data):
                    guard let details = SpiderCheckDetails(data, file: file) else {
                        self.inspectionFailed(BridgeError.message("The code-check report could not be verified. Use Reload to try again."), initial: true); return
                    }
                    self.spiderDetails = details; self.inspection.textStorage?.setAttributedString(details.attributedText)
                    self.inspection.scrollToBeginningOfDocument(nil)
                    self.pageLabel.stringValue = details.observed; self.pageLabel.toolTip = details.observedDetail
                    self.relatedEvidence.removeAllItems(); self.relatedEvidence.addItems(withTitles: ["Open File…"] + details.files)
                    self.relatedEvidence.setAccessibilityLabel("Open a code-check file")
                    self.relatedEvidence.toolTip = "Inspect a file named in this report"
                    self.relatedEvidence.isHidden = details.files.isEmpty
                    self.prepareCodeReview.isHidden = false
                case .failure(let error): self.inspectionFailed(error, initial: true)
                }
            }
        case .context(let conversation):
            prepareInspection(title: "Conversation Context", file: nil, contextSummary: true)
            inspection.font = .systemFont(ofSize: 14)
            guard canShowContext, currentConversation == conversation else {
                inspectionFailed(BridgeError.message("Conversation context is unavailable. Reconnect to a supported runtime."), initial: true); return
            }
            let generation = inspectionGeneration
            client.request("context.inspect", params: ["conversation": conversation]) { [weak self] result in
                guard let self, self.inspectionGeneration == generation, self.currentConversation == conversation else { return }
                switch result {
                case .success(let data):
                    guard let details = ConversationContextDetails(data, conversation: conversation) else {
                        self.inspectionFailed(BridgeError.message("The conversation-context report could not be verified. Use Reload to try again."), initial: true); return
                    }
                    let text = NSMutableAttributedString(string: details.text, attributes: [.font: NSFont.systemFont(ofSize: 14), .foregroundColor: NSColor.labelColor])
                    for heading in ConversationContextDetails.headings {
                        let range = (details.text as NSString).range(of: heading + "\n")
                        if range.location != NSNotFound { text.addAttribute(.font, value: NSFont.systemFont(ofSize: 14, weight: .semibold), range: range) }
                    }
                    self.inspection.textStorage?.setAttributedString(text)
                    self.inspection.scrollToBeginningOfDocument(nil)
                    self.pageLabel.stringValue = details.observed; self.pageLabel.toolTip = details.observed
                    self.contextLinks = ConversationContextLink.allCases
                    self.relatedEvidence.removeAllItems(); self.relatedEvidence.addItems(withTitles: ["Open Context…"] + self.contextLinks.map(\.title))
                    self.relatedEvidence.setAccessibilityLabel("Open a related context view")
                    self.relatedEvidence.toolTip = "Review retained messages, memories, saved state, or project runs"
                    self.relatedEvidence.isHidden = self.contextLinks.isEmpty
                case .failure(let error): self.inspectionFailed(error, initial: true)
                }
            }
        case .file(let path):
            prepareInspection(title: (path as NSString).lastPathComponent, file: path)
            openFileWithViewport(path)
        case .diff(let path, let staged):
            prepareInspection(title: "\(path) (\(staged ? "staged" : "working tree"))", file: path, diff: true)
            diffScope.selectedSegment = staged ? 1 : 0
            showPage("changes.read", params: ["path": path, "staged": staged], offset: 0, trail: [], revision: nil)
        case .output(let run):
            prepareInspection(title: "Recorded Output", file: nil)
            showPage("artifact.read", params: ["run": run], offset: 0, trail: [], revision: nil)
        case .contextHistory(let conversation):
            prepareInspection(title: "Retained History", file: nil, contextSummary: true)
            showPage("context.history", params: ["conversation": conversation], offset: 0, trail: [], revision: nil)
        case .run(let id):
            prepareInspection(title: "Run Details", file: nil, contextSummary: true)
            inspection.font = .systemFont(ofSize: 14)
            guard client.isReady, client.capabilities.contains("run-details") else {
                inspectionFailed(BridgeError.message("Run details are unavailable. Reconnect to a supported runtime."), initial: true); return
            }
            let generation = inspectionGeneration
            client.request("history.read", params: ["run": id]) { [weak self] result in
                guard let self, self.inspectionGeneration == generation else { return }
                switch result {
                case .success(let data):
                    let details = RunDetails(data)
                    self.inspection.string = details.text
                    self.inspection.scrollToBeginningOfDocument(nil)
                    self.pageLabel.stringValue = "Last recorded state · Reload to update"
                    self.pageLabel.toolTip = self.pageLabel.stringValue
                    self.evidenceLinks = details.related.filter { link in
                        switch link.kind {
                        case .activity: return self.client.capabilities.contains("walkthrough-v1")
                        case .checkpoint: return self.client.capabilities.contains("checkpoints-v1")
                        case .run: return true
                        }
                    }
                    self.relatedEvidence.removeAllItems()
                    self.relatedEvidence.addItems(withTitles: ["Related…"] + self.evidenceLinks.map(\.label))
                    self.relatedEvidence.setAccessibilityLabel("Open related evidence")
                    self.relatedEvidence.toolTip = "Review the original instruction, an activity, or a recorded checkpoint"
                    self.relatedEvidence.isHidden = self.evidenceLinks.isEmpty
                    self.recordedOutput.isHidden = !details.outputAvailable
                    self.recordedOutput.toolTip = "Inspect output recorded for this run"
                case .failure(let error): self.inspectionFailed(error, initial: true)
                }
            }
        }
    }

    func prepareInspection(title: String, file: String?, diff: Bool = false, contextSummary: Bool = false) {
        inspectionGeneration += 1
        binaryDocumentLoader?.cancel(); binaryDocumentLoader = nil
        documentView.isHidden = true; inspectionScroll.isHidden = false
        inspectionFind.isEnabled = true
        showingContextSummary = contextSummary
        let hideFind = NSMenuItem(); hideFind.tag = NSTextFinder.Action.hideFindInterface.rawValue
        inspection.performTextFinderAction(hideFind)
        inspectedFile = file; inspectedDiff = diff ? file : nil
        browsingCompact = false
        inspectorTitle.stringValue = title; inspectorTitle.toolTip = title
        inspection.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        inspection.textColor = .labelColor
        inspection.typingAttributes = [.font: NSFont.monospacedSystemFont(ofSize: 12, weight: .regular), .foregroundColor: NSColor.labelColor]
        inspection.string = "Loading…"; pageLabel.stringValue = "Loading…"; pageLabel.toolTip = nil
        recordedOutput.isHidden = true
        spiderDetails = nil; prepareCodeReview.isHidden = true; checkCode.isHidden = !canCheckCode
        savedStateDetails = nil
        relatedEvidence.isHidden = true; evidenceLinks = []; contextLinks = []
        relatedEvidence.setAccessibilityLabel("Related evidence for this instruction")
        relatedEvidence.toolTip = "Open activity, the previous instruction, or a recorded checkpoint"
        outline.isHidden = true; declarations = []
        showingInspector = true; surfaces.selectedSegment = 1
        breadcrumb.isHidden = file == nil
        if let file {
            breadcrumb.url = folder.appendingPathComponent(file)
            breadcrumb.pathItems = breadcrumb.pathItems.filter { item in
                guard let path = item.url?.path else { return false }
                return path == folder.path || path.hasPrefix(folder.path + "/")
            }
        }
        diffScope.isHidden = !diff; revealButton.isHidden = file == nil
        moreButton.isHidden = true; previousButton.isHidden = true; previousButton.isEnabled = false; nextChunk = nil; previousChunk = nil
        adapt()
    }

    func openFileWithViewport(_ path: String) {
        guard client.isReady, client.capabilities.contains("workspace-branch-v1") else {
            openInspectedFile(path, saved: nil); return
        }
        let generation = inspectionGeneration
        let source = client
        source.request("workspace.branch") { [weak self, weak source] result in
            guard let self, let source, self.client === source, self.inspectionGeneration == generation, self.inspectedFile == path else { return }
            switch result {
            case .success(let data):
                guard let branch = data["branch"] as? String else { return }
                self.viewportBranch = branch
                let kind = self.viewportKind(path)
                do {
                    let saved = try self.viewportStore.load().first { $0.branch == branch && $0.path == path && $0.kind == kind }
                    self.openInspectedFile(path, saved: saved)
                } catch {
                    self.showNotice("Could not restore the saved file position. \(error.localizedDescription)")
                    self.openInspectedFile(path, saved: nil)
                }
            case .failure(let error):
                self.showNotice("Could not identify the current branch; file position will not be saved. \(error.localizedDescription)")
                self.openInspectedFile(path, saved: nil)
            }
        }
    }

    func viewportKind(_ path: String) -> WorkspaceViewport.Kind {
        let ext = (path as NSString).pathExtension.lowercased()
        if ext == "pdf" { return .pdf }
        if ["png", "jpg", "jpeg", "gif", "tif", "tiff", "heic", "webp", "bmp"].contains(ext) { return .image }
        return ext == "csv" ? .csv : .code
    }

    func openInspectedFile(_ path: String, saved: WorkspaceViewport?) {
        let kind = viewportKind(path)
        guard [.pdf, .image].contains(kind), client.isReady, client.capabilities.contains("binary-inspection-v1") else {
            showPage("files.read", params: ["path": path], offset: saved?.pageOffset ?? 0, trail: saved?.pageTrail ?? [], revision: nil, restore: saved); return
        }
        inspectionGeneration += 1; let generation = inspectionGeneration, source = client
        let loader = BinaryDocumentLoader(path: path) { [weak source] params, completion in
            guard let source else { completion(.failure(BridgeError.message("The workspace disconnected."))); return }
            source.request("files.readBinary", params: params, completion: completion)
        }
        binaryDocumentLoader = loader
        loader.onProgress = { [weak self] count, total in
            guard let self, self.inspectionGeneration == generation else { return }
            self.pageLabel.stringValue = "Loading \(count.formatted()) / \(total.formatted()) bytes…"
        }
        loader.onComplete = { [weak self, weak loader] result in
            guard let self, let loader, self.binaryDocumentLoader === loader, self.inspectionGeneration == generation else { return }
            self.binaryDocumentLoader = nil
            switch result {
            case .failure(let error): self.inspectionFailed(error, initial: true)
            case .success(let data):
                if let branch = self.viewportBranch {
                    source.request("workspace.branch") { [weak self, weak source] result in
                        guard let self, let source, self.client === source, self.inspectionGeneration == generation else { return }
                        switch result {
                        case .success(let identity) where identity["branch"] as? String == branch:
                            self.presentDocument(data, path: path, kind: kind, saved: saved)
                        case .success: self.inspectionFailed(BridgeError.message("The branch changed while loading this document. Use Reload to open the current version."), initial: true)
                        case .failure(let error): self.inspectionFailed(error, initial: true)
                        }
                    }
                } else { self.presentDocument(data, path: path, kind: kind, saved: saved) }
            }
        }
        loader.start()
    }

    func presentDocument(_ data: Data, path: String, kind: WorkspaceViewport.Kind, saved: WorkspaceViewport?) {
        do {
            try documentView.open(data, kind: kind)
            inspectionScroll.isHidden = true; documentView.isHidden = false; inspectionFind.isEnabled = false
            window?.contentView?.layoutSubtreeIfNeeded()
            documentView.restore(saved)
            if let branch = viewportBranch { currentViewport = WorkspaceViewport(branch: branch, path: path, kind: kind) }
            pageLabel.stringValue = "\(data.count.formatted()) bytes · Read only"
        } catch { inspectionFailed(error, initial: true) }
    }

    func restoreTextViewport(_ state: WorkspaceViewport) {
        viewportRestoring = true
        defer { viewportRestoring = false }
        let length = (inspection.string as NSString).length
        let location = min(state.selectionLocation, length)
        inspection.setSelectedRange(NSRange(location: location, length: min(state.selectionLength, length - location)))
        if let container = inspection.textContainer { inspection.layoutManager?.ensureLayout(for: container) }
        window?.contentView?.layoutSubtreeIfNeeded()
        let clip = inspectionScroll.contentView
        let desired = NSRect(origin: NSPoint(x: state.scrollX, y: state.scrollY), size: clip.bounds.size)
        clip.scroll(to: clip.constrainBoundsRect(desired).origin); inspectionScroll.reflectScrolledClipView(clip)
    }

    func scheduleViewportSave() {
        guard currentViewport != nil, !viewportRestoring else { return }
        viewportSaveTimer?.invalidate()
        viewportSaveTimer = Timer.scheduledTimer(withTimeInterval: 0.4, repeats: false) { [weak self] _ in self?.saveViewport() }
    }

    func saveViewport() {
        viewportSaveTimer?.invalidate(); viewportSaveTimer = nil
        guard var state = currentViewport, !viewportRestoring else { return }
        if [.pdf, .image].contains(state.kind) { documentView.capture(into: &state) }
        else {
            let selected = inspection.selectedRange()
            state.selectionLocation = selected.location; state.selectionLength = selected.length
            state.scrollX = max(0, inspectionScroll.contentView.bounds.origin.x)
            state.scrollY = max(0, inspectionScroll.contentView.bounds.origin.y)
        }
        state.updatedAt = Date().timeIntervalSince1970
        do { try viewportStore.save(state); currentViewport = state; viewportSaveFailed = false }
        catch {
            if !viewportSaveFailed { showNotice("Could not save the file position. \(error.localizedDescription)") }
            viewportSaveFailed = true
        }
    }

    func checkViewportBranch() {
        guard !viewportBranchCheckPending, let state = currentViewport, client.isReady, client.capabilities.contains("workspace-branch-v1") else { return }
        viewportBranchCheckPending = true
        viewportBranchGeneration += 1; let generation = viewportBranchGeneration, source = client
        source.request("workspace.branch") { [weak self, weak source] result in
            guard let self, let source, self.client === source else { return }
            self.viewportBranchCheckPending = false
            guard self.viewportBranchGeneration == generation,
                  self.currentViewport?.path == state.path, self.currentViewport?.branch == state.branch else { return }
            switch result {
            case .success(let data):
                self.viewportBranchCheckFailureShown = false
                if let branch = data["branch"] as? String, branch != state.branch {
                    self.saveViewport(); self.inspect(.file(state.path), addToHistory: false)
                }
            case .failure(let error):
                if !self.viewportBranchCheckFailureShown { self.showNotice("Could not check the current branch. \(error.localizedDescription)") }
                self.viewportBranchCheckFailureShown = true
            }
        }
    }

    func textViewDidChangeSelection(_ notification: Notification) {
        if notification.object as? NSTextView === inspection { scheduleViewportSave() }
    }

    func showPage(_ method: String, params: [String: Any], offset: Int, trail: [Int], revision: String?, restore: WorkspaceViewport? = nil) {
        if method == "files.read" { saveViewport(); currentViewport = nil }
        inspectionGeneration += 1; let generation = inspectionGeneration
        var request = params; request["offset"] = offset; if let revision { request["revision"] = revision }
        if method == "files.read", let branch = viewportBranch { request["branch"] = branch }
        moreButton.isEnabled = false; previousButton.isEnabled = false
        client.request(method, params: request) { [weak self] result in
            guard let self, self.inspectionGeneration == generation else { return }
            switch result {
            case .success(let data):
                if method == "files.read", restore != nil, offset > 0, data["binary"] as? Bool == true {
                    self.showNotice("The saved position no longer matches this file. Showing its first page.")
                    self.showPage(method, params: params, offset: 0, trail: [], revision: nil); return
                }
                if method == "context.history" {
                    guard let conversation = params["conversation"] as? String, self.currentConversation == conversation else { return }
                    guard ContextHistoryPage.isValid(data, conversation: conversation, offset: offset, revision: revision) else {
                        self.inspectionFailed(BridgeError.message("The retained-history page could not be verified."), initial: revision == nil); return
                    }
                }
                let body = data["text"] as? String ?? ""
                let baseline = data["baseline"] as? String
                if method == "artifact.read" || method == "context.history" { self.inspection.font = .systemFont(ofSize: 14); self.inspection.textColor = .labelColor }
                self.inspection.string = (baseline.map { $0 + "\n\n" } ?? "") + (body.isEmpty && method == "changes.read" ? "No changes in this comparison." : body)
                if method == "files.read" || method == "changes.read" {
                    self.inspection.textStorage?.setAttributedString(CodeOutline.highlight(self.inspection.string, diff: method == "changes.read"))
                    self.declarations = method == "files.read" ? CodeOutline.declarations(in: body, path: params["path"] as? String ?? "") : []
                    self.outline.removeAllItems(); self.outline.addItem(withTitle: "Outline · declarations on this page")
                    self.outline.addItems(withTitles: self.declarations.map(\.title)); self.outline.isHidden = self.declarations.isEmpty
                }
                if method == "files.read", restore == nil { self.inspection.setSelectedRange(NSRange(location: 0, length: 0)) }
                self.inspection.scrollToBeginningOfDocument(nil)
                if method == "files.read", data["binary"] as? Bool == false,
                   let branch = self.viewportBranch, let path = params["path"] as? String {
                    let kind: WorkspaceViewport.Kind = (path as NSString).pathExtension.lowercased() == "csv" ? .csv : .code
                    self.currentViewport = WorkspaceViewport(branch: branch, path: path, kind: kind, pageOffset: offset, pageTrail: trail)
                    if let restore { self.restoreTextViewport(restore) }
                }
                let end = data["nextOffset"] as? Int ?? offset
                let size = data["size"] as? Int ?? end
                self.pageLabel.stringValue = "\(offset)–\(end) / \(size) bytes · Read only\(data["truncated"] as? Bool == true ? " · Limited" : "")"
                self.pageLabel.toolTip = self.pageLabel.stringValue
                if method == "context.history", let timestamp = data["observedAt"] as? Double {
                    let observed = DateFormatter.localizedString(from: Date(timeIntervalSince1970: timestamp / 1000), dateStyle: .short, timeStyle: .short)
                    self.pageLabel.toolTip = self.pageLabel.stringValue + " · Observed " + observed + " · Find searches this page"
                    self.pageLabel.stringValue = size == 0 ? "No messages · Read only" : "\(Int(ceil(Double(end) / 1024))) / \(Int(ceil(Double(size) / 1024))) KiB"
                }
                let token = data["revision"] as? String
                self.moreButton.isHidden = data["more"] as? Bool != true; self.moreButton.isEnabled = true
                self.nextChunk = data["more"] as? Bool == true ? { [weak self] in self?.showPage(method, params: params, offset: end, trail: Array((trail + [offset]).suffix(128)), revision: token) } : nil
                self.previousButton.isEnabled = !trail.isEmpty
                self.previousButton.isHidden = trail.isEmpty && data["more"] as? Bool != true
                self.previousChunk = trail.last.map { previous in { [weak self] in self?.showPage(method, params: params, offset: previous, trail: Array(trail.dropLast()), revision: token) } }
                if data["truncated"] as? Bool == true {
                    if method == "artifact.read" { self.showNotice("Recorded output reached its storage limit. This evidence is incomplete; review the instruction and its activity before continuing.") }
                    else if method == "changes.read" { self.showNotice("This diff exceeds the 1 MiB preview limit. The preview is incomplete; inspect the full diff with Git before reviewing it.") }
                }
            case .failure(let error):
                if method == "files.read", restore != nil, offset > 0 {
                    self.showNotice("The saved position could not be restored. Showing the first page. \(error.localizedDescription)")
                    self.showPage(method, params: params, offset: 0, trail: [], revision: nil)
                } else { self.inspectionFailed(error, initial: revision == nil) }
            }
        }
    }

    func inspectionFailed(_ error: Error, initial: Bool) {
        if initial {
            inspection.font = .systemFont(ofSize: 14)
            inspection.string = "Could not load this content.\n\n\(error.localizedDescription)\n\nUse Reload to try again, or go Back to your previous view."
            inspection.scrollToBeginningOfDocument(nil)
        }
        else { showNotice("Could not load the requested page. The last loaded page is still visible. \(error.localizedDescription)") }
        pageLabel.stringValue = initial ? "Could not load · use Reload" : "Page unchanged · use Reload"
        pageLabel.toolTip = pageLabel.stringValue
        moreButton.isHidden = true; previousButton.isEnabled = false; nextChunk = nil; previousChunk = nil
    }

    @objc func inspectBack() { if canInspectBack, let destination = inspectionHistory.back() { inspect(destination, addToHistory: false) } }

    @objc func inspectForward() { if canInspectForward, let destination = inspectionHistory.forward() { inspect(destination, addToHistory: false) } }

    @objc func findInWalkthrough() {
        guard canFindInWalkthrough else { return }
        walkthrough.focusSearch()
    }

    @objc func findInInspection() {
        if canFindInInspection {
            window?.makeFirstResponder(inspection)
            let find = NSMenuItem(); find.tag = NSTextFinder.Action.showFindInterface.rawValue
            inspection.performTextFinderAction(find)
        } else if canFindInWalkthrough {
            walkthrough.focusSearch()
        }
    }

    @objc func openRecordedOutput() {
        if case .run(let run) = inspectionHistory.current, !recordedOutput.isHidden { inspect(.output(run)) }
    }

    @objc func openRelatedEvidence() {
        let index = relatedEvidence.indexOfSelectedItem - 1
        if case .savedState(let conversation) = inspectionHistory.current {
            guard canShowSavedState, currentConversation == conversation, let details = savedStateDetails, details.report.runs.indices.contains(index) else { return }
            inspect(.run(details.report.runs[index].id)); return
        }
        if case .spider = inspectionHistory.current {
            guard client.isReady, !switchingConversation, let details = spiderDetails, details.files.indices.contains(index) else { return }
            readFile(details.files[index]); return
        }
        if case .context(let conversation) = inspectionHistory.current {
            guard canShowContext, currentConversation == conversation, contextLinks.indices.contains(index) else { return }
            relatedEvidence.selectItem(at: 0)
            switch contextLinks[index] {
            case .history: inspect(.contextHistory(conversation))
            case .memory: showMemory()
            case .savedState: showSavedState()
            case .runs: showSessions()
            case .checkpoints: showCheckpoints()
            }
            return
        }
        guard client.isReady, !switchingConversation, evidenceLinks.indices.contains(index),
              case .run = inspectionHistory.current else { return }
        let link = evidenceLinks[index]
        relatedEvidence.selectItem(at: 0)
        switch link.kind {
        case .activity: showProjectRepair(link.id)
        case .run: inspect(.run(link.id))
        case .checkpoint: openCheckpoints(selected: link.id)
        }
    }

    @objc func refreshInspection() { reloadInspection?() }

    @objc func loadNextPage() { nextChunk?() }

    @objc func loadPreviousPage() { previousChunk?() }

    @objc func changeDiffScope() { if let path = inspectedDiff { readDiff(path) } }

    @objc func revealFile() { if let path = inspectedFile { NSWorkspace.shared.activateFileViewerSelecting([folder.appendingPathComponent(path)]) } }

    @objc func openBreadcrumb() {
        guard let url = breadcrumb.clickedPathItem?.url else { return }
        let relative = url.path == folder.path ? "." : String(url.path.dropFirst(folder.path.count + 1))
        guard url.path == folder.path || url.path.hasPrefix(folder.path + "/"), url != breadcrumb.url else { return }
        navigation.select(.files, notify: false); navigate(.files); loadDirectory(relative)
    }

    @objc func jumpToDeclaration() {
        let index = outline.indexOfSelectedItem - 1
        guard declarations.indices.contains(index) else { return }
        inspection.setSelectedRange(declarations[index].range); inspection.scrollRangeToVisible(declarations[index].range)
        window?.makeFirstResponder(inspection)
    }

    func updateWalkthrough() {
        walkthrough.update(runs: runs, connected: client.isReady)
        updateWalkthroughVisibility()
        loadWalkthrough()
        probe?.walkthroughChanged(self, stage: walkthroughStage)
    }

    func moveProjectTask(_ task: ProjectTask, status: String) {
        guard client.isReady, client.capabilities.contains("project-tasks-move-v1"), !projectTaskMovePending, projectTaskDisplayedRevision >= 0 else { return }
        projectTaskMovePending = true; walkthrough.setProjectTaskMovesEnabled(false)
        let source = client
        source.request("project.tasks.move", params: ["board": task.board, "id": task.id, "status": status, "revision": projectTaskDisplayedRevision]) { [weak self, weak source] result in
            guard let self, let source, self.client === source else { return }
            self.projectTaskMovePending = false
            self.walkthrough.setProjectTaskMovesEnabled(source.isReady && source.capabilities.contains("project-tasks-move-v1"))
            if case .failure(let error) = result { self.showNotice(error.localizedDescription) }
            self.refreshProjectTasks()
        }
    }

    func refreshProjectTasks() {
        guard client.isReady, client.capabilities.contains("project-tasks-v1") else {
            walkthrough.projectTasksUnavailable(client.isReady ? "This runtime does not provide project tasks." : "Connect to LUMI to view project tasks.")
            return
        }
        if projectTasksLoading { projectTasksRefreshPending = true; return }
        projectTasksLoading = true; projectTasksRefreshPending = false
        walkthrough.setProjectTaskMovesEnabled(false); walkthrough.projectTasksLoading()
        let source = client
        let offset = projectTaskOffset
        let filters = projectTaskFilters
        let query = filters.map { ["field": $0.field.rawValue.lowercased(), "operator": $0.operation.rawValue, "value": $0.value] }
        source.request("project.tasks.list", params: ["offset": offset, "filters": query]) { [weak self, weak source] result in
            guard let self, let source, self.client === source else { return }
            self.projectTasksLoading = false
            switch result {
            case .success(let data):
                if let snapshot = ProjectTaskSnapshot(data), snapshot.revision >= self.projectTaskRevision, offset == self.projectTaskOffset, filters == self.projectTaskFilters {
                    self.projectTaskRevision = snapshot.revision; self.projectTaskDisplayedRevision = snapshot.revision; self.walkthrough.showProjectTasks(snapshot)
                    self.walkthrough.setProjectTaskMovesEnabled(source.capabilities.contains("project-tasks-move-v1") && !self.projectTaskMovePending)
                } else { self.projectTasksRefreshPending = true }
            case .failure(let error): self.walkthrough.projectTasksUnavailable(error.localizedDescription)
            }
            if self.projectTasksRefreshPending { self.refreshProjectTasks() }
        }
    }

    var walkthroughStage: String { walkthrough.diagnostics["stage"] as? String ?? "" }

    func updateWalkthroughVisibility() {
        walkthrough.setSurfaceVisible(!walkthrough.isHiddenOrHasHiddenAncestor && window?.occlusionState.contains(.visible) == true && window?.isMiniaturized != true)
    }

    func loadWalkthrough() {
        guard client.isReady, client.capabilities.contains("walkthrough-v1"), let id = walkthrough.selectedID, walkthroughLoaded != id else { return }
        walkthroughLoaded = id; walkthroughLoadGeneration += 1
        let generation = walkthroughLoadGeneration
        client.request("activity.list", params: ["run": id]) { [weak self] result in
            guard let self, generation == self.walkthroughLoadGeneration, self.walkthrough.selectedID == id else { return }
            switch result {
            case .success(let data):
                guard data["run"] as? String == id, let index = self.runs.firstIndex(where: { $0["id"] as? String == id }) else { return }
                var activityRun = self.runs[index]
                activityRun["activities"] = data["activities"]
                activityRun["activitySequence"] = data["activitySequence"]
                activityRun["activityCount"] = data["activityCount"]
                self.runs[index] = WalkthroughProjection.merged(self.runs[index], activityRun)
                self.walkthrough.showLoadError(nil); self.updateWalkthrough()
            case .failure: self.walkthrough.showLoadError("Activity could not be loaded. Recorded work is unchanged.")
            }
        }
    }
}
