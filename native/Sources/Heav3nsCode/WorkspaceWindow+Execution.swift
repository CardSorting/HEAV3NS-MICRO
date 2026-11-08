// [LAYER: UI]
import AppKit
import CryptoKit
import Foundation

extension WorkspaceWindow {
    func restoreDraft() {
        do {
            guard let draft = try draftStore.load() else { return }
            composer.string = draft.text; savedDraftText = draft.text
            currentConversation = draft.conversation; draftByConversation = draft.conversations ?? [:]
            recoverySubmission = draft.submission; savedSubmission = draft.submission
            attachmentBar.clear()
            if let attachments = draft.attachments {
                for path in attachments {
                    attachmentBar.add(url: URL(fileURLWithPath: path))
                }
            }
            savedDraftAttachments = attachmentBar.urls.map(\.path)
            if let submission = draft.submission {
                uncertainSubmission = submission
                showNotice("Draft restored. Checking whether LUMI recorded the previous instruction before you can send again.")
            } else { showNotice("Draft restored. Review it when you’re ready to send.") }
        } catch { showNotice("Could not restore the saved draft: \(error.localizedDescription) The saved file is retained at \(draftStore.file.path).") }
    }

    @discardableResult func saveDraft() -> Bool {
        draftTimer?.invalidate(); draftTimer = nil
        let currentAttachments = attachmentBar.urls.map(\.path)
        guard composer.string != savedDraftText || recoverySubmission != savedSubmission || currentAttachments != savedDraftAttachments || draftSaveFailed else { return true }
        do {
            if let currentConversation { draftByConversation[currentConversation] = composer.string }
            try draftStore.save(text: composer.string, submission: recoverySubmission, conversation: currentConversation, conversations: draftByConversation, attachments: currentAttachments)
            savedDraftText = composer.string; savedDraftAttachments = currentAttachments; savedSubmission = recoverySubmission; draftSaveFailed = false
            updateStatus(); return true
        } catch {
            draftSaveFailed = true
            showNotice("Draft could not be saved. \(error.localizedDescription) Keep this window open and copy your text before quitting.")
            updateStatus(); return false
        }
    }

    func prepareForUpdateRestart() -> Bool {
        guard !hasWork, !closing, !switchingConversation, pendingRequest == nil,
              uncertainSubmission == nil, sideQuestionPending == nil, !projectRun.running, !composer.hasMarkedText(),
              window?.attachedSheet == nil else { return false }
        return prepareToClose()
    }

    @discardableResult func prepareToClose() -> Bool {
        if let memoryWindow, memoryWindow.state.dirty || memoryWindow.state.uncertain || memoryWindow.state.saving {
            showNotice("Save or discard your memory draft before closing this workspace. You can copy it first.")
            if let window { memoryWindow.present(for: window) }; return false
        }
        return saveDraft()
    }

    @objc func submit() {
        guard !composer.hasMarkedText() else { return }
        let text = composer.string.trimmingCharacters(in: .whitespacesAndNewlines)
        if text == "/compact" { compactContext(); return }
        if let question = Self.sideQuestion(from: text) { askSideQuestion(question, original: text); return }
        submitInstruction(text)
    }

    static func sideQuestion(from text: String) -> String? {
        guard text == "/btw" || (text.hasPrefix("/btw") && text.dropFirst(4).first?.isWhitespace == true) else { return nil }
        return text.dropFirst(4).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    func askSideQuestion(_ question: String, original: String) {
        guard !question.isEmpty else { showNotice("Add a question after /btw. Your draft is preserved."); return }
        guard client.isReady, client.capabilities.contains("side-query-v1"), let conversation = currentConversation, !switchingConversation, !closing else {
            showNotice("Isolated side questions are unavailable. Your draft is preserved."); return
        }
        guard sideQuestionPending == nil else { showNotice("Wait for the current side question to finish. Your draft is preserved."); return }
        guard attachmentBar.urls.isEmpty else { showNotice("Remove attachments before asking a side question. Your draft is preserved."); return }
        guard question.utf16.count <= client.instructionLimit else { showNotice("The side question is too long. Your draft is preserved."); return }
        let id = UUID().uuidString, source = client
        sideQuestionPending = id
        sideQuestions.append(SideQuestion(id: id, conversation: conversation, question: question))
        sideQuestions = Array(sideQuestions.suffix(12))
        rebuildRows()
        var starting = true
        var immediateFailure: Error?
        source.request("side.query", params: ["conversation": conversation, "question": question], id: id) { [weak self, weak source] result in
            guard let self, let source, self.client === source else { return }
            if starting, case .failure(let error) = result { immediateFailure = error; return }
            self.sideQuestionPending = nil
            guard let index = self.sideQuestions.firstIndex(where: { $0.id == id }) else { self.updateStatus(); return }
            switch result {
            case .success(let data):
                if data["conversation"] as? String == conversation {
                    self.sideQuestions[index].answer = data["response"] as? String
                    self.sideQuestions[index].contextLimited = data["contextLimited"] as? Bool == true
                } else { self.sideQuestions[index].answer = "The answer belonged to a different conversation and was not shown."; self.sideQuestions[index].failed = true }
            case .failure(let error): self.sideQuestions[index].answer = error.localizedDescription; self.sideQuestions[index].failed = true
            }
            self.rebuildRows()
        }
        starting = false
        if let immediateFailure {
            sideQuestionPending = nil; sideQuestions.removeAll { $0.id == id }; rebuildRows(); showNotice(immediateFailure.localizedDescription); return
        }
        if composer.string.trimmingCharacters(in: .whitespacesAndNewlines) == original { composer.string = "" }
        saveDraft(); updateStatus()
    }

    func compactContext() {
        guard client.capabilities.contains("context-compact-v1"), let conversation = currentConversation else {
            showNotice("Context compaction is unavailable in this runtime. Your draft is preserved."); return
        }
        guard canRequestRecovery, pendingRequest == nil else {
            showNotice("Finish or stop current work before compacting context. Your draft is preserved."); return
        }
        guard attachmentBar.urls.isEmpty else { showNotice("Remove attachments before compacting context. Your draft is preserved."); return }
        let id = UUID().uuidString
        pendingRequest = id; updateStatus()
        client.request("context.compact", params: ["conversation": conversation], id: id) { [weak self] result in
            guard let self else { return }
            self.pendingRequest = nil
            switch result {
            case .success(let report):
                if self.currentConversation == conversation && self.composer.string.trimmingCharacters(in: .whitespacesAndNewlines) == "/compact" { self.composer.string = "" }
                let before = report["before"] as? Int ?? 0, after = report["after"] as? Int ?? 0
                self.showNotice(report["compacted"] as? Bool == true ? "Context compacted: \(before) to \(after) active messages. Recorded conversation remains available." : "Context is already minimal. Recorded conversation remains available.")
            case .failure(let error): self.showNotice(error.localizedDescription)
            }
            self.saveDraft(); self.updateStatus()
        }
    }

    var canRequestRecovery: Bool { client.isReady && !hasWork && !switchingConversation && uncertainSubmission == nil && !closing && !cancelRequested }
    var canContinueWork: Bool { canRequestRecovery && client.capabilities.contains("task-continuation-v1") }

    func continueWork(_ continuation: WorkspaceContinuation) {
        guard canContinueWork, WorkspaceContinuation.latest(in: runs) == continuation else { return }
        submitInstruction(continuation.instruction, continuation: continuation)
    }

    func repairProject(_ launch: String) {
        guard canRequestRecovery, client.capabilities.contains("project-repair-v1"), UUID(uuidString: launch) != nil else { return }
        submitInstruction("Investigate and fix why the app stopped. Use its recorded failure, preserve existing work, and run the project checks. Tell me what is ready and whether app startup has been confirmed.", repairLaunch: launch)
    }

    func showProjectRepair(_ id: String) {
        guard client.isReady, !switchingConversation, !closing else { return }
        if runs.contains(where: { $0["id"] as? String == id }) { showWalkthrough(); walkthrough.selectRecordedRun(id); return }
        let conversation = currentConversation
        client.request("history.read", params: ["run": id]) { [weak self] result in
            guard let self, self.currentConversation == conversation, !self.switchingConversation else { return }
            switch result {
            case .success(let data):
                if let run = data["run"] as? [String: Any] { self.mergeRun(run); self.rebuildRows(); self.showWalkthrough(); self.walkthrough.selectRecordedRun(id) }
            case .failure(let error): self.showNotice(error.localizedDescription)
            }
        }
    }

    func submitInstruction(_ text: String, continuation: WorkspaceContinuation? = nil, repairLaunch: String? = nil) {
        guard !reasoningPicker.pending else { showNotice("Wait for the reasoning setting to finish saving. Your draft is preserved."); return }
        guard !text.isEmpty, pendingRequest == nil, !switchingConversation else { return }
        guard text.utf16.count <= client.instructionLimit else { showNotice("Keep this instruction under \(client.instructionLimit.formatted()) characters; your draft is preserved."); return }
        guard client.isReady, uncertainSubmission == nil, !closing else { showNotice("Reconnect to check the last confirmed state before sending. Your draft is preserved."); return }
        guard !cancelRequested else { showNotice("LUMI is still cancelling. Wait for its confirmed stopping boundary."); return }
        guard runs.filter({ $0["state"] as? String == "queued" }).count < client.queueLimit else { showNotice("The queue is full. Your draft is preserved until a slot opens. Review waiting instructions in Up Next."); return }
        let repairID = repairLaunch.map { launch in "repair-" + SHA256.hash(data: Data("\(currentConversation ?? ""):\(launch)".utf8)).map { String(format: "%02x", $0) }.joined() }
        let id = repairID ?? continuation.map { "continue-\($0.run)-\($0.revision)" } ?? UUID().uuidString
        let submission = WorkspaceDraft.Submission(id: id, text: text, preservesDraft: continuation != nil || repairLaunch != nil)
        recoverySubmission = submission
        guard saveDraft() else { recoverySubmission = nil; return }
        pendingRequest = id; updateStatus()
        var parameters: [String: Any] = ["text": text, "attachments": attachmentBar.urls.map(\.path)]
        if client.capabilities.contains("conversations"), let currentConversation { parameters["conversation"] = currentConversation }
        if let continuation { parameters["continueFrom"] = ["run": continuation.run, "revision": continuation.revision] }
        if let repairLaunch { parameters["repairProject"] = ["launch": repairLaunch] }
        client.request("task.submit", params: parameters, id: id) { [weak self] result in
            guard let self else { return }; self.pendingRequest = nil
            switch result {
            case .success(let data):
                self.recoverySubmission = nil
                if let run = data["run"] as? [String: Any] { self.mergeRun(run) }
                if self.cancelRequested, !self.client.capabilities.contains("workspace-stop"), let run = data["run"] as? [String: Any], let runID = run["id"] as? String {
                    self.client.request("task.cancel", params: ["run": runID]) { [weak self] result in
                        if case .failure(let error) = result { self?.showNotice(error.localizedDescription) }
                    }
                }
                if submission.preservesDraft != true, self.composer.string.trimmingCharacters(in: .whitespacesAndNewlines) == text {
                    self.composer.string = ""
                    self.attachmentBar.clear()
                }
                self.rebuildRows()
                self.walkthrough.followCurrentInstruction()
                if repairLaunch != nil, let run = data["run"] as? [String: Any], let id = run["id"] as? String { self.showProjectRepair(id) }
            case .failure(let error):
                if (error as? RuntimeFailure)?.acceptanceUnknown == true {
                    self.uncertainSubmission = submission
                    self.showNotice("Acceptance is unknown. Reconnect to check whether LUMI recorded this instruction. Your draft is preserved.")
                } else { self.recoverySubmission = nil; self.showNotice(error.localizedDescription) }
            }
            self.saveDraft()
            self.updateStatus()
        }
    }

    func reconcileSubmission() {
        guard client.isReady, !closing, !checkingSubmission, let uncertain = uncertainSubmission else { return }
        checkingSubmission = true; updateStatus()
        client.request("task.lookup", params: ["request": uncertain.id]) { [weak self] result in
            guard let self, self.uncertainSubmission?.id == uncertain.id else { return }
            self.checkingSubmission = false
            switch result {
            case .success(let data):
                self.uncertainSubmission = nil
                self.recoverySubmission = nil
                if data["found"] as? Bool == true, let run = data["run"] as? [String: Any] {
                    self.mergeRun(run)
                    if uncertain.preservesDraft != true, self.composer.string.trimmingCharacters(in: .whitespacesAndNewlines) == uncertain.text {
                        self.composer.string = ""
                        self.attachmentBar.clear()
                    }
                    self.showNotice("LUMI recorded the instruction. Its recovered state is shown below; it was not resubmitted.")
                    self.rebuildRows()
                    self.walkthrough.followCurrentInstruction()
                } else { self.showNotice("LUMI has no acceptance record for that instruction. Review your draft and send it when ready.") }
                self.saveDraft()
            case .failure(let error): self.showNotice(self.client.isReady ? "Could not confirm the instruction yet. Choose Check Status to retry. Your draft is preserved and nothing will be resubmitted." : error.localizedDescription)
            }
            self.updateStatus()
        }
    }

    @objc func reconnect() {
        if client.isReady, uncertainSubmission != nil { reconcileSubmission(); return }
        guard client.process?.isRunning != true, !closing else { return }
        client = RuntimeClient(); podContextID = UUID(); decidingApproval = nil; approvalFailure = nil
        stopped = false; sequence = 0; cancelRequested = false; stopAcknowledged = true; cancellationToken = nil; forceAvailable = false; recovering = true
        configureClient(); reconnectButton.isHidden = true; client.start(folder: folder)
    }

    func updateApproval() {
        if let decidingApproval, decidingApproval.request != currentApproval || decidingApproval.context != podContextID { self.decidingApproval = nil; approvalFailure = nil }
        if let failure = approvalFailureAttempt, failure.request != currentApproval || failure.context != podContextID { approvalFailure = nil; approvalFailureAttempt = nil }
        approval.isHidden = currentApproval == nil
        reviewApprovalButton.isEnabled = canReviewApproval
        denyButton.isEnabled = canDecideApproval
        allowButton.isEnabled = canDecideApproval
        let summary = currentApproval.map { "\($0.tool)\n\($0.scope)" } ?? ""
        if approvalText.string != summary { approvalText.string = summary }
        if let review = approvalReview {
            review.update(isCurrent: currentApproval == review.request && approvalReviewContext == podContextID, connected: client.isReady, submitting: decidingApproval?.request == review.request, stopping: cancelRequested || closing || switchingConversation, error: approvalFailure)
        }
    }

    func updateStatus() {
        memoryWindow?.syncAvailability()
        reasoningPicker.refresh(supported: client.isReady && client.capabilities.contains("reasoning-selection-v1"))
        modelButton.isEnabled = canChooseModels
        modelButton.toolTip = client.capabilities.contains("model-selection-v1") ? modelButton.toolTip : "This runtime does not support model switching. Rebuild the app and runtime together."
        modelPicker?.refreshAvailability()
        checkpointsButton.isEnabled = client.isReady && client.capabilities.contains("checkpoints-v1") && !switchingConversation
        checkpointWindow?.refreshAvailability()
        composer.needsDisplay = true
        composer.isEditable = !switchingConversation
        openFileButton.isEnabled = canOpenFile
        updateApproval()
        if !hasWork && !closing && stopAcknowledged { forceAvailable = false; cancelRequested = false; cancellationToken = nil; stopButton.title = "Stop" }
        let queued = runs.filter { $0["state"] as? String == "queued" }.count
        let needsLookup = client.isReady && uncertainSubmission != nil
        reconnectButton.title = needsLookup ? (checkingSubmission ? "Checking…" : "Check Status") : "Reconnect"
        reconnectButton.isEnabled = !checkingSubmission && !closing
        reconnectButton.isHidden = closing || (!needsLookup && (client.isReady || client.process?.isRunning == true))
        let draft = composer.string.trimmingCharacters(in: .whitespacesAndNewlines)
        let side = Self.sideQuestion(from: draft)
        sendButton.isEnabled = side != nil ? (client.isReady && !switchingConversation && !closing && sideQuestionPending == nil && draft.utf16.count <= client.instructionLimit) : (!reasoningPicker.pending && client.isReady && !switchingConversation && pendingRequest == nil && uncertainSubmission == nil && !closing && !cancelRequested && !draft.isEmpty && draft.utf16.count <= client.instructionLimit && queued < client.queueLimit)
        stopButton.isEnabled = forceAvailable || (!cancelRequested && hasWork) || (closing && !stopped)
        let run = runs.first(where: { $0["id"] as? String == active })
        let state = run?["state"] as? String
        let startAnchor = (run?["startedAt"] as? Double) ?? (state == "running" ? run?["createdAt"] as? Double : nil)
        let elapsedSeconds = startAnchor.map { Int(max(0, Date().timeIntervalSince1970 - $0 / 1000)) }
        let elapsed = elapsedSeconds.map { " · \($0)s" } ?? ""
        let condition = client.isReady ? "Connected" : connection == "Connecting to LUMI…" ? "Connecting…" : connection == "Preparing" ? "Preparing…" : "Disconnected"
        let activeParsed = run.map(WalkthroughProjection.activities) ?? []
        let activeCurrent = activeParsed.filter { $0.isActive && $0.scope != "turn" }.max { $0.sequence < $1.sequence } ?? activeParsed.max { $0.sequence < $1.sequence }
        let liveStage = state == "running" ? (activeCurrent?.stage ?? "Starting") : nil
        let batchSummary = state == "running" ? WalkthroughProjection.progressSummary(activeParsed, live: true) : ""
        let latestTurnDetail = state == "running" ? activeParsed.filter { $0.scope == "turn" && !$0.detail.isEmpty }.max { $0.sequence < $1.sequence }?.detail : nil
        let waitingDetail: String? = state == "running" ? (latestTurnDetail ?? (activeParsed.isEmpty && (elapsedSeconds ?? 0) >= 3 ? "Waiting \(elapsedSeconds ?? 0)s for the first activity event…" : nil)) : nil
        let activity = decidingApproval != nil ? "Saving decision…" : state == "awaiting_approval" ? "Waiting for your review" : liveStage.map { "Running · \($0)" } ?? state?.replacingOccurrences(of: "_", with: " ").capitalized
        let deferBadge = (client.isReady && swarmDeferred) ? " · Deferred" : ""
        status.stringValue = pendingRequest != nil ? "Submitting…" : "\(condition)\(cancelRequested ? " · Cancelling" : activity.map { " · " + $0 } ?? (sideQuestionPending != nil ? " · Answering side question…" : ""))\(deferBadge)\(elapsed)\(queued > 0 ? " · \(queued) queued" : "")"
        status.toolTip = status.stringValue
        let chatActivity = activity.map { $0 + elapsed + (batchSummary.isEmpty ? "" : " · " + batchSummary) + (waitingDetail.map { " · " + $0 } ?? "") }
        chatState.stringValue = switchingConversation ? "Opening conversation…" : pendingRequest != nil ? "Submitting instruction…" : chatActivity ?? (sideQuestionPending != nil ? "Answering side question…" : runs.isEmpty ? "Tell LUMI what you want to build." : "Recorded conversation · send a follow-up to continue")
        chatState.toolTip = chatState.stringValue
        conversations.setCanSwitch(canSwitchConversation)
        sendButton.title = side != nil ? "Ask" : hasWork ? "Queue" : "Send"
        stopSendButton.isHidden = side != nil || !hasWork || draft.isEmpty
        stopSendButton.isEnabled = client.isReady && pendingRequest == nil && uncertainSubmission == nil && !cancelRequested && stopThenSend == nil && draft.utf16.count <= client.instructionLimit
        hint.stringValue = draftSaveFailed ? "Draft not saved · keep this window open" : draft.utf16.count > client.instructionLimit ? "Instruction too long · draft preserved" : side != nil ? "Side question · temporary · no tools" : queued >= client.queueLimit ? "Queue full · draft preserved" : uncertainSubmission != nil ? (client.isReady ? "Acceptance unknown · Check Status before sending" : "Acceptance unknown · reconnect to reconcile") : client.capabilities.contains("autonomous-execution-v1") ? "Autonomous · Enter to send · Stop anytime" : "Enter to send · Shift–Enter for a new line"
        hint.toolTip = side != nil ? "Side questions use a copy of recent context, run no tools, and do not stop the active instruction." : hint.stringValue + "\nEnter to send · Shift–Enter for a new line. Use Stop to interrupt."
        queueButton.title = queued > 0 ? "Up Next (\(queued))" : "Up Next"
        queueButton.setAccessibilityLabel("Up Next, \(queued) queued instructions")
        updateQueue()
        updateElapsedTimer()
        walkthrough.tick()
        walkthrough.setRecoveryAvailability(supported: client.capabilities.contains("task-continuation-v1"), enabled: canContinueWork,
            hint: !client.isReady ? "Reconnect to continue the saved work." : hasWork ? "Finish or stop current work before continuing." : uncertainSubmission != nil ? "Reconnect to confirm the previous submission first." : "Continue from the saved changes and run fresh project checks. Your draft is preserved.")
        projectRun.setRepairAvailability(supported: client.capabilities.contains("project-repair-v1"), enabled: canRequestRecovery,
            canOpen: client.isReady && !switchingConversation && !closing, conversation: currentConversation,
            hint: !client.isReady ? "Reconnect to repair the app." : hasWork ? "Finish or stop current work before starting a repair." : uncertainSubmission != nil ? "Reconnect to confirm the previous submission first." : "LUMI will investigate the saved failure and check the project. Your draft is preserved.", automaticSupported: client.capabilities.contains("project-self-healing-v1"))
        updateWalkthroughVisibility()
    }

    func updateElapsedTimer() {
        let needed = (active != nil || pendingRequest != nil || sideQuestionPending != nil) && client.isReady && window?.occlusionState.contains(.visible) == true && window?.isMiniaturized != true
        if needed && elapsedTimer == nil { elapsedTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.updateStatus() } }
        else if !needed { elapsedTimer?.invalidate(); elapsedTimer = nil }
    }

    func showNotice(_ text: String) { notice.stringValue = text; noticeBar.isHidden = false }

    @objc func dismissNotice() { if client.isReady && uncertainSubmission == nil { noticeBar.isHidden = true } }

    @objc func stop() {
        if forceAvailable { stopThenSend = nil; client.forceStop(); showNotice("Force Stop interrupted LUMI. Existing edits remain; remote effects may be unknown."); return }
        guard !cancelRequested else { return }
        let source = client
        let token = UUID(); cancellationToken = token; cancelRequested = true
        stopAcknowledged = !client.capabilities.contains("workspace-stop"); updateStatus()
        if client.capabilities.contains("workspace-stop") {
            client.request("task.cancelAll", params: [:]) { [weak self] result in
                guard let self, self.client === source, self.cancellationToken == token else { return }
                switch result {
                case .success:
                    self.stopAcknowledged = true
                    if self.closing { self.updateStatus(); return }
                    source.requestSnapshot(afterPending: true) { [weak self] in
                        guard let self, self.client === source, self.cancellationToken == token else { return }
                        self.updateStatus(); self.finishStopAndSend()
                    }
                case .failure(let error):
                    self.showNotice(error.localizedDescription); self.stopThenSend = nil
                    if source.isReady {
                        self.cancelRequested = false; self.cancellationToken = nil; self.stopAcknowledged = true
                        self.forceAvailable = false; self.stopButton.title = "Stop"
                    }
                    self.updateStatus()
                }
            }
        } else {
            for run in runs where !terminalStates.contains(run["state"] as? String ?? "") {
                if let id = run["id"] as? String { client.request("task.cancel", params: ["run": id]) { [weak self] result in
                    guard let self, self.client === source, self.cancellationToken == token else { return }
                    if case .failure(let error) = result { self.showNotice(error.localizedDescription); self.stopThenSend = nil }
                } }
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) { [weak self] in
            guard let self, self.client === source, self.cancellationToken == token, self.cancelRequested, self.hasWork || (self.closing && !self.stopped) else { return }
            self.forceAvailable = true; self.stopButton.title = "Force Stop"; self.stopButton.isEnabled = true
        }
    }

    @objc func stopAndSend() {
        let text = composer.string.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty, text.utf16.count <= client.instructionLimit, !composer.hasMarkedText(), pendingRequest == nil, client.isReady, stopThenSend == nil else { return }
        let ids = Set(runs.filter { !terminalStates.contains($0["state"] as? String ?? "") }.compactMap { $0["id"] as? String })
        guard !ids.isEmpty else { submit(); return }
        stopThenSend = (text, ids); stop()
    }

    func finishStopAndSend() {
        guard let pending = stopThenSend, stopAcknowledged else { return }
        let targets = runs.filter { pending.runs.contains($0["id"] as? String ?? "") }
        if targets.contains(where: { $0["state"] as? String == "interrupted" }) {
            stopThenSend = nil; showNotice("The stopping boundary is unknown. Review the interrupted run before sending your preserved draft."); return
        }
        guard targets.count == pending.runs.count, targets.allSatisfy({ terminalStates.contains($0["state"] as? String ?? "") }), !hasWork else { return }
        stopThenSend = nil; cancelRequested = false; cancellationToken = nil; submitInstruction(pending.text)
    }

    func updateQueue() {
        if let editing = editingQueued, queueEditor?.window?.sheetParent != nil {
            let run = runs.first { $0["id"] as? String == editing.id }
            let reason: String?
            if !client.isReady { reason = "LUMI is disconnected. Your edit is preserved; use Copy Edit to keep it while reconnecting." }
            else if cancelRequested || closing { reason = "Work is stopping. Your edit is preserved; use Copy Edit to keep it for a follow-up." }
            else if run?["state"] as? String != "queued" { reason = "This instruction is no longer waiting. Your edit is preserved; use Copy Edit to keep it for a follow-up." }
            else if run?["revision"] as? Int != editing.revision { reason = "This instruction changed while you were editing. Use Copy Edit, then close and reopen it to review the latest version." }
            else { reason = nil }
            queueEditor?.setUnavailable(reason)
        }
        let entries = runs.filter { $0["state"] as? String == "queued" }.sorted {
            ($0["queueOrder"] as? Double ?? $0["createdAt"] as? Double ?? 0) < ($1["queueOrder"] as? Double ?? $1["createdAt"] as? Double ?? 0)
        }.compactMap { run -> QueuedInstruction? in
            guard let id = run["id"] as? String, let text = run["text"] as? String else { return nil }
            return QueuedInstruction(id: id, text: text, revision: run["revision"] as? Int ?? 0)
        }
        queueContent.update(entries, enabled: client.isReady && !cancelRequested && !closing, editing: client.capabilities.contains("queue-edit"), removing: client.capabilities.contains("queue-remove"), busy: removingQueued)
        if queuePopover.isShown { queuePopover.contentSize = queueContent.preferredContentSize }
    }

    @objc func showQueue() {
        guard window?.attachedSheet == nil else { return }
        updateQueue()
        if queuePopover.isShown { queuePopover.close() }
        else { queuePopover.show(relativeTo: queueButton.bounds, of: queueButton, preferredEdge: .maxY) }
    }

    func removeQueued(_ entry: QueuedInstruction) {
        guard client.isReady, !cancelRequested, !closing, !removingQueued.contains(entry.id) else { return }
        removingQueued.insert(entry.id); updateQueue()
        client.request("task.cancel", params: ["run": entry.id, "ifQueued": true]) { [weak self] result in
            guard let self else { return }
            self.removingQueued.remove(entry.id)
            if case .failure(let error) = result { self.showNotice(error.localizedDescription) }
            self.updateQueue(); self.client.requestSnapshot()
        }
    }

    func editQueued(_ entry: QueuedInstruction) {
        guard client.isReady, !cancelRequested, !closing, window?.attachedSheet == nil else { return }
        queuePopover.close()
        client.request("history.read", params: ["run": entry.id]) { [weak self] result in
            guard let self, let window = self.window, window.attachedSheet == nil else { return }
            switch result {
            case .failure(let error): self.showNotice(error.localizedDescription)
            case .success(let data):
                guard let run = data["run"] as? [String: Any], run["state"] as? String == "queued", let text = run["text"] as? String, let revision = run["revision"] as? Int else {
                    self.showNotice("This instruction has already left the queue. Follow its progress in the transcript."); self.client.requestSnapshot(); return
                }
                self.mergeRun(run)
                self.editingQueued = QueuedInstruction(id: entry.id, text: text, revision: revision)
                self.queueEditor = QueuedInstructionEditor(text: text, limit: self.client.instructionLimit) { [weak self] text, complete in
                    guard let self, self.client.isReady, !self.cancelRequested, !self.closing else { complete(.failure(BridgeError.message("Connect to LUMI and wait for cancellation to finish before saving."))); return }
                    self.client.request("task.edit", params: ["run": entry.id, "text": text, "revision": revision]) { [weak self] result in
                        switch result {
                        case .failure(let error): complete(.failure(error)); self?.client.requestSnapshot()
                        case .success(let data):
                            if let run = data["run"] as? [String: Any] { self?.mergeRun(run); self?.rebuildRows() }
                            complete(.success(()))
                        }
                    }
                }
                self.queueEditor?.present(for: window)
                self.updateQueue()
            }
        }
    }

    @objc func showApprovalReview() {
        guard canReviewApproval, let request = currentApproval, let window else { return }
        let context = podContextID; approvalReviewContext = context
        approvalReview = ApprovalReview(request: request, decide: { [weak self] request, decision in
            guard let self, self.podContextID == context else { return }
            self.resolveApproval(request, decision: decision)
        }, didClose: { [weak self] in self?.approvalReview = nil; self?.updateApproval() })
        approvalReview?.present(for: window); updateApproval()
    }

    @objc func allowOnce() { if let request = currentApproval { resolveApproval(request, decision: "allow") } }
    @objc func deny() { if let request = currentApproval { resolveApproval(request, decision: "deny") } }

    var canDecideApproval: Bool { currentApproval != nil && client.isReady && decidingApproval == nil && !cancelRequested && !closing && !switchingConversation }

    @discardableResult func resolveApproval(_ approval: ApprovalRequest, decision: String) -> Bool {
        guard ["allow", "deny"].contains(decision), currentApproval == approval, client.isReady, decidingApproval == nil, !cancelRequested, !closing, !switchingConversation else { return false }
        let source = client, attempt = ApprovalDecision(request: approval, context: podContextID)
        decidingApproval = attempt; approvalFailure = nil; approvalFailureAttempt = nil; updateStatus()
        source.request("approval.resolve", params: ["operation": approval.operation, "run": approval.run, "decision": decision]) { [weak self] result in
            guard let self, self.client === source, self.podContextID == attempt.context, self.decidingApproval == attempt else { return }
            if case .failure(let error) = result {
                self.approvalFailureAttempt = attempt
                if source.isReady { self.decidingApproval = nil; self.approvalFailure = error.localizedDescription }
                else { self.approvalFailure = "Decision outcome unknown. Reconnect to recover recorded state before deciding again." }
                self.showNotice(self.approvalFailure ?? error.localizedDescription)
            }
            if source.isReady { source.requestSnapshot(afterPending: true) }
            self.updateStatus()
        }
        return decidingApproval == attempt
    }

    func openCheckpoints(selected: String?) {
        guard client.isReady, client.capabilities.contains("checkpoints-v1"), !switchingConversation, let window, window.attachedSheet == nil else { return }
        quickOpen?.dismiss(restoreFocus: false)
        if let existing = checkpointWindow, existing.window?.isVisible == true { if let selected { existing.selectCheckpoint(selected) }; existing.window?.makeKeyAndOrderFront(nil); return }
        checkpointWindow = CheckpointWindow(request: { [weak self] method, params, complete in self?.client.request(method, params: params, completion: complete) }, selected: selected, canRestore: { [weak self] in
            guard let self else { return false }; return self.canSwitchConversation && !self.projectRun.running
        }, restore: { [weak self] id, token, complete in self?.restoreCheckpoint(id, token: token, complete: complete) })
        checkpointWindow?.present(for: window)
    }

    func restoreCheckpoint(_ id: String, token: String, complete: @escaping (Result<Void, Error>) -> Void) {
        guard canSwitchConversation, saveDraft() else { complete(.failure(BridgeError.message("Finish or stop LUMI before restoring. Your draft is preserved."))); return }
        switchingConversation = true; expectedConversation = nil
        navigationGeneration += 1; inspectionGeneration += 1; conversationGeneration += 1; walkthroughLoadGeneration += 1; updateStatus()
        client.request("checkpoint.restore", params: ["id": id, "token": token]) { [weak self] result in
            guard let self else { return }
            switch result {
            case .failure(let error): self.switchingConversation = false; self.expectedConversation = nil; self.updateStatus(); complete(.failure(error))
            case .success(let data):
                self.expectedConversation = data["conversation"] as? String
                self.client.requestSnapshot(afterPending: true) { [weak self] in
                    guard let self else { return }
                    guard self.currentConversation == self.expectedConversation else { self.client.closeInput(); complete(.failure(BridgeError.message("Restore could not be confirmed. Reconnect before sending."))); return }
                    self.switchingConversation = false; self.expectedConversation = nil
                    self.showNotice(data["message"] as? String ?? "Restored in a new conversation."); self.loadConversations(); self.refreshNavigation(); self.updateStatus(); complete(.success(())); self.focusComposer()
                }
            }
        }
    }
}

