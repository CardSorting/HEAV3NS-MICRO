// [LAYER: UI]
import AppKit
import Foundation

extension WorkspaceWindow {
    @objc func toggleAgentDefer() {
        let method = swarmDeferred ? "task.resume" : "task.defer"
        client.request(method, params: [:]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                switch result {
                case .success(let payload):
                    self.swarmDeferred = method == "task.defer"
                    let boundary = payload["boundary"] as? String == "before_turn" ? "before the next turn" : "at a safe boundary"
                    self.showNotice(self.swarmDeferred ? "Agent work is deferred \(boundary)." : "Agent work resumed.")
                    self.updateStatus()
                case .failure(let error): self.showNotice(error.localizedDescription)
                }
            }
        }
    }

    @objc func requestCriticalReview() {
        let text = "[AGENT_SWARM_CRITICAL_REVIEW] Perform one bounded, evidence-based critical completion review of the latest completed work in this conversation. Check original requirements, architecture and constraints, verification quality, and concrete failure modes. Distinguish demonstrated defects, unverified assumptions, and optional improvements. Do not invent defects or start an unlimited task chain. End with exactly one decision: ACCEPT, REMEDIATE, ESCALATE, or ACCEPT WITH DISCLOSED LIMITATIONS, with concise evidence. This is one review pass only."
        client.request("task.submit", params: ["text": text, "attachments": []]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                if case .success = result { self.showNotice("Critical review queued.") }
                else if case .failure(let error) = result { self.showNotice(error.localizedDescription) }
            }
        }
    }

    @objc func engageSpecialists() {
        let alert = NSAlert(); alert.messageText = "Engage Specialist Team"; alert.informativeText = "Describe the objective for independent specialist agents. Their reports return to this coordinator."
        alert.addButton(withTitle: "Engage"); alert.addButton(withTitle: "Cancel")
        let objective = NSTextField(string: ""); objective.placeholderString = "Objective"
        objective.frame = NSRect(x: 0, y: 0, width: 360, height: 28); alert.accessoryView = objective
        guard alert.runModal() == .alertFirstButtonReturn, !objective.stringValue.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        let task = objective.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        client.request("agentSwarm.classify", params: ["objective": task]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                switch result {
                case .success(let plan): self.confirmSpecialistPlan(task, plan: plan)
                case .failure(let error): self.showNotice(error.localizedDescription)
                }
            }
        }
    }

    func confirmSpecialistPlan(_ objective: String, plan: [String: Any]) {
        guard let assignments = plan["assignments"] as? [[String: Any]], !assignments.isEmpty else { showNotice("The task classifier returned no specialist assignments."); return }
        let scores = plan["scores"] as? [[String: Any]] ?? []
        let purposes: [String: String] = [
            "Architect": "Plans the approach and how the pieces fit together.",
            "Researcher": "Investigates relevant files and documentation.",
            "Implementer": "Makes the requested changes.",
            "UX Specialist": "Reviews the experience, navigation, and accessibility.",
            "Hardener": "Checks safety, edge cases, and failure handling.",
            "Data Steward": "Checks how information is stored and updated.",
            "Verifier": "Checks the result against the request."
        ]
        let roleByID = assignments.reduce(into: [String: String]()) { roles, assignment in
            guard let id = assignment["id"] as? String, let role = assignment["role"] as? String else { return }
            roles[id] = role
        }
        let filesByRole = scores.reduce(into: [String: [String]]()) { files, score in
            guard let role = score["role"] as? String else { return }
            files[role] = Array((score["assignedFiles"] as? [String] ?? []).prefix(3))
        }
        let steps = assignments.enumerated().map { index, assignment in
            let role = assignment["role"] as? String ?? "Specialist"
            let dependencies = (assignment["dependsOn"] as? [String] ?? []).compactMap { roleByID[$0] }
            let focus = filesByRole[role] ?? []
            var lines = ["\(index + 1). \(role) — \(purposes[role] ?? "Works on the assigned part of the request.")"]
            if !focus.isEmpty { lines.append("   Focus: \(focus.joined(separator: ", "))") }
            if !dependencies.isEmpty { lines.append("   Starts after: \(dependencies.joined(separator: ", "))") }
            return lines.joined(separator: "\n")
        }.joined(separator: "\n")
        let strategyNames: [String: String] = [
            "adaptive": "Adaptive team",
            "tactical_strike": "Focused team",
            "balanced_squad": "Balanced team",
            "full_legion": "Full specialist team",
            "verification_heavy": "Verification-focused team"
        ]
        let intentNames: [String: String] = [
            "CREATE": "Create something new", "FIX": "Fix an issue", "REFACTOR": "Improve existing work",
            "INVESTIGATE": "Investigate a question", "TEST": "Verify a result", "CONFIGURE": "Change settings",
            "DELETE": "Remove something", "GENERAL": "General request"
        ]
        let confirm = NSAlert(); confirm.messageText = "Review Specialist Plan"
        confirm.informativeText = "Your request\n\(objective.prefix(700))\n\n\(intentNames[plan["intent"] as? String ?? ""] ?? "General request") · \(strategyNames[plan["strategy"] as? String ?? ""] ?? "Adaptive team") · \(assignments.count) specialists\n\nTeam plan\n\(steps)\n\nStarting gives specialists access to workspace tools. They may edit files or run commands. Review their activity and reports, then inspect the workspace changes."
        confirm.addButton(withTitle: "Start Team"); confirm.addButton(withTitle: "Cancel")
        guard confirm.runModal() == .alertFirstButtonReturn else { return }
        client.request("agentSwarm.form", params: ["objective": objective, "assignments": assignments]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                switch result {
                case .success(let report):
                    let deferred = report["status"] as? String == "deferred"
                    let failed = report["status"] as? String == "partial_failure"
                    self.showNotice("Specialist formation \(deferred ? "deferred with its remaining assignments recorded" : failed ? "finished with blockers" : "finished").\n\n\(self.formationRoadmapText(report))")
                case .failure(let error): self.showNotice(error.localizedDescription)
                }
            }
        }
    }

    static func mergeFormationAssignments(savedAssignments: [[String: Any]], roadmapAssignments: [[String: Any]]) -> [[String: Any]] {
        guard !savedAssignments.isEmpty else { return roadmapAssignments }
        let statesById = Dictionary(roadmapAssignments.compactMap { state -> (String, [String: Any])? in
            guard let id = state["id"] as? String, !id.isEmpty else { return nil }
            return (id, state)
        }, uniquingKeysWith: { _, latest in latest })
        var seenIds = Set<String>()
        var merged = savedAssignments.map { saved -> [String: Any] in
            guard let id = saved["id"] as? String, !id.isEmpty else { return saved }
            seenIds.insert(id)
            guard let state = statesById[id] else { return saved }
            var combined = saved.merging(state) { _, roadmapValue in roadmapValue }
            combined["id"] = id
            if let role = saved["role"] as? String, !role.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                combined["role"] = role
            }
            if let objective = saved["objective"] as? String, !objective.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                combined["objective"] = objective
            }
            if let dependsOn = saved["dependsOn"] as? [String], !dependsOn.isEmpty {
                combined["dependsOn"] = dependsOn
            }
            return combined
        }
        for state in roadmapAssignments {
            if let id = state["id"] as? String, !id.isEmpty, !seenIds.contains(id) {
                merged.append(state)
            }
        }
        return merged
    }

    struct FormationHistoryReviewModel {
        let status: String
        let formationId: String?
        let automatic: Bool
        let assignments: [[String: Any]]
        let canRecover: Bool
        let canResume: Bool
        let canInspect: Bool
        let buttonTitles: [String]

        init(entry: [String: Any]) {
            let status = entry["status"] as? String ?? "unknown"
            let formationId = entry["id"] as? String
            let automatic = entry["automatic"] as? Bool == true && entry["parentRunQueued"] as? Bool != false
            let savedAssignments = entry["assignments"] as? [[String: Any]] ?? []
            let roadmapAssignments = (entry["roadmap"] as? [String: Any])?["assignments"] as? [[String: Any]]
                ?? (entry["result"] as? [String: Any])?["reports"] as? [[String: Any]]
                ?? entry["reports"] as? [[String: Any]]
                ?? []
            let assignments = WorkspaceWindow.mergeFormationAssignments(savedAssignments: savedAssignments, roadmapAssignments: roadmapAssignments)
            let canRecover = status == "needs_reconciliation" && formationId != nil && !assignments.isEmpty
            let canResume = status == "deferred" && formationId != nil
            let canInspect = status != "none" && status != "missing" && formationId != nil && !assignments.isEmpty
            var titles = [String]()
            if canRecover {
                titles.append("Review Recovery…")
            } else if canResume {
                titles.append(automatic ? "Resume Run" : "Resume Formation")
            }
            if canInspect {
                titles.append("Inspect Saved Stream…")
            }
            titles.append("Close")
            self.status = status
            self.formationId = formationId
            self.automatic = automatic
            self.assignments = assignments
            self.canRecover = canRecover
            self.canResume = canResume
            self.canInspect = canInspect
            self.buttonTitles = titles
        }
    }

    static func inspectableSpecialists(from status: [String: Any]) -> [[String: Any]] {
        let savedAssignments = status["assignments"] as? [[String: Any]] ?? []
        let roadmapAssignments = (status["roadmap"] as? [String: Any])?["assignments"] as? [[String: Any]]
            ?? (status["result"] as? [String: Any])?["reports"] as? [[String: Any]]
            ?? status["reports"] as? [[String: Any]]
            ?? []
        let merged = mergeFormationAssignments(savedAssignments: savedAssignments, roadmapAssignments: roadmapAssignments)
        let activeOrSavedStatuses: Set<String> = ["completed", "deferred", "failed", "running", "needs_reconciliation", "reconciling", "partial_failure"]
        let withActivity = merged.filter { assignment in
            guard let id = assignment["id"] as? String, !id.isEmpty else { return false }
            if let childSession = assignment["childSessionId"] as? String, !childSession.isEmpty { return true }
            if let state = assignment["status"] as? String, activeOrSavedStatuses.contains(state) { return true }
            if let summary = assignment["summary"] as? String, !summary.isEmpty { return true }
            if let response = assignment["response"] as? String, !response.isEmpty { return true }
            if let reason = assignment["reason"] as? String, !reason.isEmpty { return true }
            return false
        }
        if !withActivity.isEmpty { return withActivity }
        return merged.filter { assignment in
            guard let id = assignment["id"] as? String, !id.isEmpty else { return false }
            let state = assignment["status"] as? String ?? ""
            return state != "pending" && state != "skipped"
        }
    }

    @objc func showFormationStatus() {
        client.request("agentSwarm.formStatus", params: [:]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                let alert = NSAlert(); alert.messageText = "Specialist Formation Status"
                var formationId: String?
                var isAutomaticFormation = false
                var canResume = false
                var canInspect = false
                var canReconcile = false
                var streamAssignments = [[String: Any]]()
                switch result {
                case .success(let status):
                    alert.informativeText = self.formationRoadmapText(status)
                    formationId = status["id"] as? String
                    isAutomaticFormation = status["automatic"] as? Bool == true && status["parentRunQueued"] as? Bool != false
                    canResume = status["status"] as? String == "deferred" && formationId != nil
                    canReconcile = status["status"] as? String == "needs_reconciliation" && formationId != nil
                    let savedAssignments = status["assignments"] as? [[String: Any]] ?? []
                    let roadmapAssignments = (status["roadmap"] as? [String: Any])?["assignments"] as? [[String: Any]]
                        ?? (status["result"] as? [String: Any])?["reports"] as? [[String: Any]]
                        ?? status["reports"] as? [[String: Any]]
                        ?? []
                    streamAssignments = Self.mergeFormationAssignments(savedAssignments: savedAssignments, roadmapAssignments: roadmapAssignments)
                    let currentStatus = status["status"] as? String ?? "none"
                    canInspect = currentStatus != "none" && currentStatus != "missing" && formationId != nil && !streamAssignments.isEmpty
                case .failure(let error): alert.informativeText = error.localizedDescription
                }
                if canReconcile {
                    alert.addButton(withTitle: "Reconcile Interrupted Work…")
                } else if canResume {
                    alert.addButton(withTitle: isAutomaticFormation ? "Resume Run" : "Resume Formation")
                }
                if canInspect { alert.addButton(withTitle: "Inspect Saved Stream…") }
                alert.addButton(withTitle: "Close")
                let response = alert.runModal()
                guard let formationId else { return }
                let hasPrimaryAction = canReconcile || canResume
                if response == .alertFirstButtonReturn {
                    if canReconcile {
                        self.beginFormationReconciliation(formationId: formationId, assignments: streamAssignments)
                    } else if canResume {
                        self.resumeFormation(formationId, automatic: isAutomaticFormation)
                    } else if canInspect {
                        self.chooseInterruptedFormationStream(formationId: formationId, assignments: streamAssignments)
                    }
                } else if response == .alertSecondButtonReturn, hasPrimaryAction, canInspect {
                    self.chooseInterruptedFormationStream(formationId: formationId, assignments: streamAssignments)
                }
            }
        }
    }

    func beginFormationReconciliation(formationId: String, assignments: [[String: Any]]) {
        let uncertain = assignments.filter { assignment in
            !["completed", "pending", "skipped"].contains(assignment["status"] as? String ?? "unknown")
        }
        guard !uncertain.isEmpty else {
            showNotice("No specialist has an unresolved roadmap state. Refresh Formation Status and inspect its saved record.")
            return
        }
        collectFormationReconciliationDecision(formationId: formationId, assignments: uncertain, index: 0, decisions: []) { [weak self] decisions in
            guard let self else { return }
            let confirmation = NSTextField(frame: NSRect(x: 0, y: 0, width: 420, height: 24))
            confirmation.placeholderString = "Briefly describe what you inspected (at least 20 characters)"
            confirmation.setAccessibilityLabel("Workspace and terminal review notes")
            let stopped = NSButton(checkboxWithTitle: "I confirmed prior specialist activity is stopped and reviewed its workspace and terminal effects.", target: nil, action: nil)
            stopped.setAccessibilityLabel("Confirm prior specialist activity is stopped and reviewed")
            let stack = NSStackView(views: [stopped, confirmation])
            stack.orientation = .vertical
            stack.alignment = .leading
            stack.spacing = 10
            stack.frame = NSRect(x: 0, y: 0, width: 440, height: 66)
            let final = NSAlert()
            final.messageText = "Confirm Formation Reconciliation"
            final.informativeText = "This records your decisions and prepares the formation for an explicit resume. It does not start specialists. Verify previous activity has stopped and review its effects before confirming."
            final.accessoryView = stack
            final.addButton(withTitle: "Reconcile and Prepare Resume")
            final.addButton(withTitle: "Cancel")
            guard final.runModal() == .alertFirstButtonReturn else { return }
            let note = confirmation.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard stopped.state == .on, note.count >= 20 else {
                self.showNotice("Reconciliation was not submitted. Confirm the stopped and reviewed checkbox and add at least 20 characters describing your review.")
                return
            }
            self.client.request("agentSwarm.formReconcile", params: ["formationId": formationId, "confirmation": note, "confirmedStopped": true, "decisions": decisions]) { result in
                DispatchQueue.main.async {
                    switch result {
                    case .success(let receipt):
                        let isAutomatic = receipt["automatic"] as? Bool == true
                        let parentQueued = receipt["parentRunQueued"] as? Bool == true
                        if parentQueued { self.swarmDeferred = true; self.updateStatus() }
                        let nextStep = isAutomatic
                            ? (parentQueued ? "\n\nChoose Resume Run to continue the original task from its reviewed specialist reports." : "\n\nThe original task is not queued. Resume the formation here, then start a new continuation if needed.")
                            : ""
                        self.showNotice("Interrupted work reconciled. The formation is now deferred and has not been started. Use Formation Status to choose the next step.\n\n\(self.formationRoadmapText(receipt))\(nextStep)")
                    case .failure(let error): self.showNotice(error.localizedDescription)
                    }
                }
            }
        }
    }

    func collectFormationReconciliationDecision(formationId: String, assignments: [[String: Any]], index: Int, decisions: [[String: Any]], completion: @escaping ([[String: Any]]) -> Void) {
        guard index < assignments.count else { completion(decisions); return }
        let assignment = assignments[index]
        guard let specialistId = assignment["id"] as? String else {
            showNotice("A saved specialist entry is missing its ID. Reconciliation cannot continue safely.")
            return
        }
        let state = assignment["status"] as? String ?? "unknown"
        let choices: [(String, String)] = [("retry", "Retry in a fresh specialist session"), ("completed", "Mark completed with a saved stream and summary")] + (state == "deferred" ? [("resume", "Resume from the saved checkpoint")] : [])
        let picker = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 390, height: 26))
        picker.addItems(withTitles: choices.map(\.1))
        picker.setAccessibilityLabel("Recovery decision for \(assignment["role"] as? String ?? specialistId)")
        let summary = NSTextField(frame: NSRect(x: 0, y: 0, width: 390, height: 48))
        summary.placeholderString = "Evidence summary (required for completed work)"
        summary.setAccessibilityLabel("Completed specialist evidence summary")
        let stack = NSStackView(views: [picker, summary])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.frame = NSRect(x: 0, y: 0, width: 400, height: 82)
        let prompt = NSAlert()
        prompt.messageText = "Reconcile \(assignment["role"] as? String ?? specialistId)"
        prompt.informativeText = "Saved state: \(state). Retry starts only after you later resume. Mark completed only after reviewing a saved stream. Resume is available only for a saved deferred checkpoint."
        prompt.accessoryView = stack
        prompt.addButton(withTitle: "Save Decision")
        prompt.addButton(withTitle: "Cancel")
        guard prompt.runModal() == .alertFirstButtonReturn else { return }
        let choice = choices[min(max(picker.indexOfSelectedItem, 0), choices.count - 1)].0
        let evidence = summary.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        if choice == "completed" && evidence.count < 12 {
            showNotice("Add an evidence summary of at least 12 characters before marking this specialist completed.")
            return
        }
        if choice == "completed" {
            client.request("agentSwarm.formStream", params: ["formationId": formationId, "specialistId": specialistId]) { [weak self] result in
                DispatchQueue.main.async {
                    guard let self else { return }
                    guard case .success(let stream) = result else {
                        if case .failure(let error) = result { self.showNotice("A saved stream is required to mark this specialist completed.\n\n\(error.localizedDescription)") }
                        return
                    }
                    let detail = NSAlert()
                    detail.messageText = "Review Saved Stream · \(assignment["role"] as? String ?? specialistId)"
                    let excerpt = (stream["excerpt"] as? String ?? "No transcript excerpt was saved.").prefix(12_000)
                    detail.informativeText = "\(stream["messageCount"] as? Int ?? 0) saved messages. This excerpt may not include work after the last checkpoint. Compare it with current workspace and terminal effects before marking the work complete.\n\n\(excerpt)"
                    detail.addButton(withTitle: "I Reviewed This Stream")
                    detail.addButton(withTitle: "Cancel")
                    guard detail.runModal() == .alertFirstButtonReturn else { return }
                    self.collectFormationReconciliationDecision(formationId: formationId, assignments: assignments, index: index + 1, decisions: decisions + [["specialistId": specialistId, "outcome": choice, "summary": evidence]], completion: completion)
                }
            }
        } else {
            collectFormationReconciliationDecision(formationId: formationId, assignments: assignments, index: index + 1, decisions: decisions + [["specialistId": specialistId, "outcome": choice]], completion: completion)
        }
    }

    func chooseInterruptedFormationStream(formationId: String, assignments: [[String: Any]]) {
        let choices = assignments.compactMap { assignment -> (id: String, title: String)? in
            guard let id = assignment["id"] as? String, !id.isEmpty else { return nil }
            let role = assignment["role"] as? String ?? id
            let state = assignment["status"] as? String ?? "unknown"
            return (id, "\(role) · \(state.replacingOccurrences(of: "_", with: " ").capitalized)")
        }
        guard !choices.isEmpty else {
            showNotice("This formation has no saved specialist stream records. Inspect the workspace and terminal before deciding how to continue.")
            return
        }

        let alert = NSAlert()
        alert.messageText = "Inspect Saved Specialist Stream"
        alert.informativeText = "Choose a saved stream to review. The excerpt is limited evidence; inspecting it does not resume or replay any specialist work. Verify workspace and terminal effects before deciding what to do next."
        alert.addButton(withTitle: "Inspect Stream")
        alert.addButton(withTitle: "Cancel")
        let picker = NSPopUpButton()
        picker.addItems(withTitles: choices.map { $0.title })
        picker.setAccessibilityLabel("Saved specialist stream to inspect")
        picker.frame = NSRect(x: 0, y: 0, width: 380, height: 26)
        alert.accessoryView = picker
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        let choice = choices[min(max(picker.indexOfSelectedItem, 0), choices.count - 1)]
        client.request("agentSwarm.formStream", params: ["formationId": formationId, "specialistId": choice.id]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                switch result {
                case .success(let stream):
                    let excerpt = stream["excerpt"] as? String ?? "No transcript excerpt was saved."
                    let count = stream["messageCount"] as? Int ?? 0
                    let childSession = stream["childSessionId"] as? String ?? "unknown"
                    let detail = NSAlert()
                    detail.messageText = "Saved Stream · \(choice.title)"
                    detail.informativeText = "Session: \(childSession) · \(count) saved messages\n\n\(excerpt)\n\nThis saved excerpt may not include work performed after its last checkpoint. Inspect the current workspace and terminal before retrying or continuing."
                    detail.addButton(withTitle: "Close")
                    detail.runModal()
                case .failure(let error):
                    self.showNotice("No saved checkpoint is available for this specialist. Inspect the workspace and terminal directly before deciding how to continue.\n\n\(error.localizedDescription)")
                }
            }
        }
    }

    func resumeFormation(_ formationId: String, automatic: Bool) {
        client.request("agentSwarm.settings", params: ["action": "get"]) { [weak self] settingsResult in
            DispatchQueue.main.async {
                guard let self else { return }
                guard case .success(let settings) = settingsResult else {
                    if case .failure(let error) = settingsResult { self.showNotice(error.localizedDescription) }
                    return
                }
                let resumeFormationWork = {
                    self.client.request("agentSwarm.formResume", params: ["formationId": formationId]) { result in
                        DispatchQueue.main.async {
                            switch result {
                            case .success(let report): self.showNotice("Formation resumed.\n\n\(self.formationRoadmapText(report))")
                            case .failure(let error): self.showNotice(error.localizedDescription)
                            }
                        }
                    }
                }
                if automatic || self.swarmDeferred || settings["enabled"] as? Bool == true || settings["hierarchicalDefer"] as? Bool == true {
                    self.client.request("task.resume", params: [:]) { resumed in
                        DispatchQueue.main.async {
                            guard case .success = resumed else {
                                if case .failure(let error) = resumed { self.showNotice(error.localizedDescription) }
                                return
                            }
                            self.swarmDeferred = false
                            self.updateStatus()
                            if automatic { self.showNotice("Hierarchy resumed. The parent run will continue from its saved specialist reports.") }
                            else { resumeFormationWork() }
                        }
                    }
                } else { resumeFormationWork() }
            }
        }
    }

    @objc func showFormationHistory() {
        client.request("agentSwarm.formHistory", params: ["limit": 20]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                switch result {
                case .success(let history):
                    let entries = history["formations"] as? [[String: Any]] ?? []
                    guard !entries.isEmpty else {
                        self.showNotice("No specialist work has been recorded yet.")
                        return
                    }
                    let titleFor: ([String: Any]) -> String = { entry in
                        let status = self.formationStatusLabel(entry["status"] as? String ?? "unknown")
                        let objective = (entry["objective"] as? String ?? "Specialist work").replacingOccurrences(of: "\n", with: " ")
                        let shortObjective = String(objective.prefix(72)) + (objective.count > 72 ? "…" : "")
                        return "\(status) · \(shortObjective)"
                    }
                    let picker = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 420, height: 26))
                    picker.addItems(withTitles: entries.map(titleFor))
                    picker.setAccessibilityLabel("Specialist work to review")
                    let alert = NSAlert()
                    alert.messageText = "Specialist Work"
                    alert.informativeText = "Choose a saved formation to review its progress and available next step."
                    alert.accessoryView = picker
                    alert.addButton(withTitle: "Review Selection…")
                    alert.addButton(withTitle: "Close")
                    guard alert.runModal() == .alertFirstButtonReturn else { return }
                    let entry = entries[min(max(picker.indexOfSelectedItem, 0), entries.count - 1)]
                    self.reviewFormationHistoryEntry(entry)
                case .failure(let error): self.showNotice(error.localizedDescription)
                }
            }
        }
    }

    func reviewFormationHistoryEntry(_ entry: [String: Any]) {
        let model = FormationHistoryReviewModel(entry: entry)
        let detail = NSAlert()
        detail.messageText = "Specialist Work · \(formationStatusLabel(model.status))"
        detail.informativeText = formationRoadmapText(entry)
        for title in model.buttonTitles {
            detail.addButton(withTitle: title)
        }
        let response = detail.runModal()
        guard let formationId = model.formationId else { return }
        let hasPrimaryAction = model.canRecover || model.canResume
        if response == .alertFirstButtonReturn {
            if model.canRecover {
                beginFormationReconciliation(formationId: formationId, assignments: model.assignments)
            } else if model.canResume {
                resumeFormation(formationId, automatic: model.automatic)
            } else if model.canInspect {
                chooseInterruptedFormationStream(formationId: formationId, assignments: model.assignments)
            }
        } else if response == .alertSecondButtonReturn, hasPrimaryAction, model.canInspect {
            chooseInterruptedFormationStream(formationId: formationId, assignments: model.assignments)
        }
    }

    @objc func showLatestSpecialistStream() {
        client.request("agentSwarm.formStatus", params: [:]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                guard case .success(let status) = result else { self.showNotice("Could not load specialist formation status."); return }
                let currentStatus = status["status"] as? String ?? "none"
                let formationId = status["id"] as? String
                let inspectable = Self.inspectableSpecialists(from: status)
                guard currentStatus != "none", currentStatus != "missing", let formationId, !inspectable.isEmpty else {
                    self.showNotice("The latest formation has no saved specialist stream to inspect.")
                    return
                }
                if inspectable.count > 1 {
                    self.chooseInterruptedFormationStream(formationId: formationId, assignments: inspectable)
                    return
                }
                guard let specialist = inspectable.last, let specialistId = specialist["id"] as? String else {
                    self.showNotice("The latest formation has no saved specialist stream to inspect.")
                    return
                }
                self.client.request("agentSwarm.formStream", params: ["formationId": formationId, "specialistId": specialistId]) { streamResult in
                    DispatchQueue.main.async {
                        switch streamResult {
                        case .success(let stream):
                            let excerpt = stream["excerpt"] as? String ?? "No transcript excerpt was saved."
                            let count = stream["messageCount"] as? Int ?? 0
                            let alert = NSAlert(); alert.messageText = "Specialist Stream · \(specialist["role"] as? String ?? specialistId)"
                            alert.informativeText = "Showing the latest excerpt from \(count) saved messages.\n\n\(excerpt)"
                            alert.addButton(withTitle: "OK"); alert.runModal()
                        case .failure(let error): self.showNotice(error.localizedDescription)
                        }
                    }
                }
            }
        }
    }

    func formationRoadmapText(_ value: Any) -> String {
        guard let record = value as? [String: Any] else { return String(describing: value).prefix(6000).description }
        var lines: [String] = []
        let status = record["status"] as? String ?? "unknown"
        lines.append("Status: \(formationStatusLabel(status))")
        if let objective = record["objective"] as? String, !objective.isEmpty { lines.append("Objective: \(objective)") }
        if let reason = record["reason"] as? String, !reason.isEmpty { lines.append("Recovery: \(reason)") }
        if let error = record["error"] as? String, !error.isEmpty { lines.append("Error: \(error)") }
        if let roadmap = record["roadmap"] as? [String: Any] {
            if let sync = roadmap["synchronizationStatus"] as? String { lines.append("Synchronization: \(sync.replacingOccurrences(of: "_", with: " "))") }
            if let completed = roadmap["completedWork"] as? [String], !completed.isEmpty { lines.append("Completed: \(completed.joined(separator: ", "))") }
            if let outstanding = roadmap["outstandingWork"] as? [String], !outstanding.isEmpty { lines.append("Outstanding: \(outstanding.joined(separator: ", "))") }
            if let assignments = roadmap["assignments"] as? [[String: Any]], !assignments.isEmpty {
                lines.append("\nSpecialists")
                for assignment in assignments {
                    let id = assignment["id"] as? String ?? "specialist"
                    let role = assignment["role"] as? String ?? id
                    let state = assignment["status"] as? String ?? "pending"
                    let dependencies = assignment["dependsOn"] as? [String] ?? []
                    lines.append("• \(role) · \(formationStatusLabel(state))\(dependencies.isEmpty ? "" : " · after \(dependencies.joined(separator: ", "))")")
                    if let replacement = assignment["replacementRole"] as? String { lines.append("  Rerouted to: \(replacement) · \(assignment["attempts"] as? Int ?? 2) attempts") }
                    if let session = assignment["childSessionId"] as? String { lines.append("  Stream: \(session)") }
                    if let reason = assignment["reason"] as? String, !reason.isEmpty { lines.append("  Blocker: \(reason)") }
                    if let summary = assignment["summary"] as? String, !summary.isEmpty { lines.append("  Report: \(summary)") }
                }
            }
            if let blockers = roadmap["blockers"] as? [String], !blockers.isEmpty { lines.append("\nBlockers\n" + blockers.map { "• \($0)" }.joined(separator: "\n")) }
            if let actions = roadmap["nextActions"] as? [String], !actions.isEmpty { lines.append("\nNext actions\n" + actions.map { "• \($0)" }.joined(separator: "\n")) }
        } else if let result = record["result"] as? [String: Any], let reports = result["reports"] as? [[String: Any]], !reports.isEmpty {
            lines.append("\nSpecialist reports")
            for report in reports {
                let role = report["role"] as? String ?? report["id"] as? String ?? "Specialist"
                let state = report["status"] as? String ?? "unknown"
                lines.append("• \(role) · \(state.capitalized)")
                if let reason = report["reason"] as? String { lines.append("  Blocker: \(reason)") }
                if let response = report["response"] as? String, !response.isEmpty { lines.append("  Report: \(response)") }
            }
        }
        if status == "needs_reconciliation" { lines.append("\nInspect child streams and workspace effects before retrying. Uncertain operations are not replayed automatically.") }
        return lines.joined(separator: "\n").prefix(6000).description
    }

    func formationStatusLabel(_ status: String) -> String {
        switch status {
        case "completed": return "Finished"
        case "deferred": return "Paused"
        case "partial_failure": return "Finished with issues"
        case "needs_reconciliation": return "Needs review"
        case "reconciling": return "Recovery in progress"
        case "running": return "In progress"
        case "pending": return "Waiting"
        case "failed": return "Needs attention"
        case "skipped": return "Skipped"
        default: return status.replacingOccurrences(of: "_", with: " ").capitalized
        }
    }
}
