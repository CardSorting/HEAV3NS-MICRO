// [LAYER: UI]
import AppKit
import Foundation

extension WorkspaceWindow {
    @objc func showCommandRecovery() {
        client.request("command.actionsList", params: [:]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                guard case .success(let response) = result else {
                    if case .failure(let error) = result { self.showNotice(error.localizedDescription) }
                    return
                }
                let actions = response["actions"] as? [[String: Any]] ?? []
                guard !actions.isEmpty else {
                    self.showNotice("No unresolved command actions. Completed command receipts stay available for replay protection; this view only lists actions that still need an outcome.")
                    return
                }
                let truncated = response["truncated"] as? Bool == true
                let choices = actions.enumerated().map { index, action -> String in
                    let state = (action["status"] as? String ?? "unknown").capitalized
                    let fingerprint = String((action["commandFingerprint"] as? String ?? "").prefix(12))
                    let active = action["ownerActive"] as? Bool == true ? " · active here" : " · prior process"
                    return "\(index + 1). \(state) · command \(fingerprint)\(active)"
                }
                let picker = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 390, height: 26))
                picker.addItems(withTitles: choices)
                picker.setAccessibilityLabel("Unresolved command action")
                let review = NSAlert()
                review.messageText = "Unresolved Command Actions"
                review.informativeText = "Review the original terminal before recording an outcome. These receipts store a command fingerprint, not the command text. An action still owned by this process stays fenced.\(truncated ? " Showing the 100 most recent of \(response["total"] as? Int ?? actions.count) receipts." : "")"
                review.accessoryView = picker
                review.addButton(withTitle: "Review Selected…")
                review.addButton(withTitle: "Close")
                guard review.runModal() == .alertFirstButtonReturn else { return }
                let action = actions[min(max(picker.indexOfSelectedItem, 0), actions.count - 1)]
                guard let actionId = action["actionId"] as? String else { self.showNotice("This receipt is incomplete and cannot be reconciled safely."); return }
                if action["ownerActive"] as? Bool == true {
                    self.showNotice("This action is still owned by the current process and remains fenced. Wait for that process to settle, or reopen the workspace after it exits, then inspect the terminal before reconciling.")
                    return
                }
                self.confirmCommandActionOutcome(action: action, actionId: actionId)
            }
        }
    }

    func confirmCommandActionOutcome(action: [String: Any], actionId: String) {
        let status = action["status"] as? String ?? "unknown"
        let fingerprint = action["commandFingerprint"] as? String ?? "unavailable"
        let date = (action["updatedAt"] as? NSNumber).map { Date(timeIntervalSince1970: $0.doubleValue / 1000) }
        let updated = date.map { DateFormatter.localizedString(from: $0, dateStyle: .medium, timeStyle: .short) } ?? "unknown"
        let confirmation = NSTextField(frame: NSRect(x: 0, y: 0, width: 410, height: 24))
        confirmation.placeholderString = "Terminal evidence checked; do not include secrets (12+ characters)"
        confirmation.setAccessibilityLabel("Terminal evidence for command outcome")
        let stopped = NSButton(checkboxWithTitle: "I confirmed the original command process is no longer running.", target: nil, action: nil)
        stopped.setAccessibilityLabel("Confirm original command process has stopped")
        let outcome = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 410, height: 26))
        outcome.addItems(withTitles: ["Completed successfully", "Failed or stopped"])
        outcome.setAccessibilityLabel("Verified command outcome")
        let stack = NSStackView(views: [stopped, outcome, confirmation])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.frame = NSRect(x: 0, y: 0, width: 420, height: 86)
        let alert = NSAlert()
        alert.messageText = "Record Verified Command Outcome"
        alert.informativeText = "Status: \(status) · Updated: \(updated)\nFingerprint: \(String(fingerprint.prefix(24)))…\n\nA stopped worker does not prove its command process exited. Inspect the original terminal and workspace, then confirm the command process has stopped. Recording an outcome removes the retry fence; it never replays the command."
        alert.accessoryView = stack
        alert.addButton(withTitle: "Record Outcome")
        alert.addButton(withTitle: "Cancel")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        let note = confirmation.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard stopped.state == .on, note.count >= 12 else {
            showNotice("Outcome was not recorded. Confirm the command has stopped and add at least 12 characters describing the terminal evidence you checked.")
            return
        }
        let resultName = outcome.indexOfSelectedItem == 0 ? "completed" : "failed"
        client.request("command.actionReconcile", params: ["actionId": actionId, "outcome": resultName, "confirmation": note, "confirmedStopped": true]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                switch result {
                case .success: self.showNotice("Command outcome recorded as \(resultName). No command was dispatched.")
                case .failure(let error): self.showNotice(error.localizedDescription)
                }
            }
        }
    }

    @objc func showAgentSwarmSettings() {
        client.request("agentSwarm.settings", params: ["action": "get"]) { [weak self] result in
            DispatchQueue.main.async {
                guard let self else { return }
                guard case .success(let values) = result else { self.showNotice("Could not load Agent Swarm settings."); return }
                let alert = NSAlert()
                alert.messageText = "Agent Swarm Settings"
                alert.informativeText = "Turning on the master switch enables all features without changing your individual preferences. Turn the master switch off to customize individual features right here. Choose whether a pause waits for the current turn or stops after the current tool batch. Automatic review pausing requires both Critical completion review and Hierarchical defer. These settings are shared by CLI and the native app on this machine."
                alert.addButton(withTitle: "Save")
                alert.addButton(withTitle: "Cancel")
                final class MasterToggleController: NSObject {
                    let update: () -> Void
                    init(update: @escaping () -> Void) { self.update = update }
                    @objc func masterDidToggle(_ sender: NSButton) { update() }
                }
                let master = NSButton(checkboxWithTitle: "Enable Agent Swarm", target: nil, action: nil)
                let review = NSButton(checkboxWithTitle: "Critical completion review", target: nil, action: nil)
                let autoDeferReview = NSButton(checkboxWithTitle: "Pause specialist work during review", target: nil, action: nil)
                let initialReviewLimit = NSPopUpButton(); initialReviewLimit.addItems(withTitles: (1...5).map(String.init)); initialReviewLimit.selectItem(withTitle: String(values["maxInitialReviews"] as? Int ?? 1))
                let correctivePassLimit = NSPopUpButton(); correctivePassLimit.addItems(withTitles: (1...5).map(String.init)); correctivePassLimit.selectItem(withTitle: String(values["maxCorrectivePasses"] as? Int ?? 1))
                let deferToggle = NSButton(checkboxWithTitle: "Hierarchical defer and resume", target: nil, action: nil)
                let legion = NSButton(checkboxWithTitle: "Legion specialist delegation", target: nil, action: nil)
                let specialistCap = NSPopUpButton()
                specialistCap.addItems(withTitles: (1...8).map(String.init))
                specialistCap.selectItem(withTitle: String(values["maxSpecialists"] as? Int ?? 4))
                let boundary = NSPopUpButton()
                let boundaryOptions = [("safe_boundary", "Finish the current tool batch"), ("before_turn", "Finish the current turn")]
                boundary.addItems(withTitles: boundaryOptions.map { $0.1 })
                boundary.selectItem(withTitle: boundaryOptions.first(where: { $0.0 == (values["deferBoundary"] as? String ?? "safe_boundary") })?.1 ?? boundaryOptions[0].1)
                let strategy = NSPopUpButton()
                let strategyOptions = [
                    ("adaptive", "Adaptive (recommended)"),
                    ("tactical_strike", "Tactical strike"),
                    ("balanced_squad", "Balanced squad"),
                    ("full_legion", "Full Legion"),
                    ("verification_heavy", "Verification heavy"),
                ]
                strategy.addItems(withTitles: strategyOptions.map { $0.1 })
                strategy.selectItem(withTitle: strategyOptions.first(where: { $0.0 == (values["strategy"] as? String ?? "adaptive") })?.1 ?? strategyOptions[0].1)
                let masterEnabled = values["enabled"] as? Bool == true
                let environmentOverrides = Set(values["environmentOverrides"] as? [String] ?? [])
                if !environmentOverrides.isEmpty {
                    let settingNames = [
                        "enabled": "Agent Swarm",
                        "criticalCompletionReview": "Critical completion review",
                        "maxInitialReviews": "Initial review limit",
                        "maxCorrectivePasses": "Corrective review limit",
                        "autoDeferOnCompletionReview": "Pause during completion review",
                        "hierarchicalDefer": "Defer and resume",
                        "legion": "Specialist delegation",
                        "deferBoundary": "Pause timing",
                        "strategy": "Delegation strategy",
                        "maxSpecialists": "Maximum specialists",
                    ]
                    let lockedNames = environmentOverrides.compactMap { settingNames[$0] }.sorted().joined(separator: ", ")
                    alert.informativeText += "\n\nManaged by environment settings and read-only here: \(lockedNames)."
                }
                let savedReviewState: NSControl.StateValue = values["criticalCompletionReview"] as? Bool == true ? .on : .off
                let savedDeferState: NSControl.StateValue = values["hierarchicalDefer"] as? Bool == true ? .on : .off
                let savedLegionState: NSControl.StateValue = values["legion"] as? Bool == true ? .on : .off
                master.state = masterEnabled ? .on : .off
                review.state = !environmentOverrides.contains("criticalCompletionReview") && masterEnabled ? .on : savedReviewState
                autoDeferReview.state = values["autoDeferOnCompletionReview"] as? Bool != false ? .on : .off
                deferToggle.state = !environmentOverrides.contains("hierarchicalDefer") && masterEnabled ? .on : savedDeferState
                legion.state = !environmentOverrides.contains("legion") && masterEnabled ? .on : savedLegionState
                let capLabel = NSTextField(labelWithString: "Maximum specialists")
                let capRow = NSStackView(views: [capLabel, specialistCap]); capRow.orientation = .horizontal; capRow.alignment = .centerY; capRow.spacing = 10
                let initialReviewRow = NSStackView(views: [NSTextField(labelWithString: "Initial reviews per completion"), initialReviewLimit]); initialReviewRow.orientation = .horizontal; initialReviewRow.alignment = .centerY; initialReviewRow.spacing = 10
                let correctivePassRow = NSStackView(views: [NSTextField(labelWithString: "Corrective verification passes"), correctivePassLimit]); correctivePassRow.orientation = .horizontal; correctivePassRow.alignment = .centerY; correctivePassRow.spacing = 10
                let boundaryLabel = NSTextField(labelWithString: "Pause after")
                let boundaryRow = NSStackView(views: [boundaryLabel, boundary]); boundaryRow.orientation = .horizontal; boundaryRow.alignment = .centerY; boundaryRow.spacing = 10
                let strategyLabel = NSTextField(labelWithString: "Delegation strategy")
                let strategyRow = NSStackView(views: [strategyLabel, strategy]); strategyRow.orientation = .horizontal; strategyRow.alignment = .centerY; strategyRow.spacing = 10
                let stack = NSStackView(views: [master, review, initialReviewRow, correctivePassRow, autoDeferReview, deferToggle, boundaryRow, legion, strategyRow, capRow])
                stack.orientation = .vertical; stack.alignment = .leading; stack.spacing = 8
                for (control, label) in [(boundary, "Choose when a defer request pauses active work"), (strategy, "Choose how specialist assignments are selected"), (specialistCap, "Limit the number of specialists in a formation"), (initialReviewLimit, "Set the maximum initial reviews for one completion"), (correctivePassLimit, "Set the maximum corrective verification reviews")] {
                    control.setAccessibilityLabel(label)
                }
                var previousMasterState = master.state == .on
                let syncToggleEnabledStates = {
                    let isMasterOn = master.state == .on
                    if isMasterOn && !previousMasterState {
                        if !environmentOverrides.contains("criticalCompletionReview") { review.state = .on }
                        if !environmentOverrides.contains("hierarchicalDefer") { deferToggle.state = .on }
                        if !environmentOverrides.contains("legion") { legion.state = .on }
                    } else if !isMasterOn && previousMasterState && !masterEnabled {
                        if !environmentOverrides.contains("criticalCompletionReview") { review.state = savedReviewState }
                        if !environmentOverrides.contains("hierarchicalDefer") { deferToggle.state = savedDeferState }
                        if !environmentOverrides.contains("legion") { legion.state = savedLegionState }
                    }
                    previousMasterState = isMasterOn
                    review.isEnabled = !environmentOverrides.contains("criticalCompletionReview") && !isMasterOn
                    deferToggle.isEnabled = !environmentOverrides.contains("hierarchicalDefer") && !isMasterOn
                    legion.isEnabled = !environmentOverrides.contains("legion") && !isMasterOn
                }
                let toggleController = MasterToggleController(update: syncToggleEnabledStates)
                master.target = toggleController
                master.action = #selector(MasterToggleController.masterDidToggle(_:))
                master.isEnabled = !environmentOverrides.contains("enabled")
                syncToggleEnabledStates()
                initialReviewLimit.isEnabled = !environmentOverrides.contains("maxInitialReviews")
                correctivePassLimit.isEnabled = !environmentOverrides.contains("maxCorrectivePasses")
                autoDeferReview.isEnabled = !environmentOverrides.contains("autoDeferOnCompletionReview")
                boundary.isEnabled = !environmentOverrides.contains("deferBoundary")
                strategy.isEnabled = !environmentOverrides.contains("strategy")
                specialistCap.isEnabled = !environmentOverrides.contains("maxSpecialists")
                alert.accessoryView = stack
                let modalResult = withExtendedLifetime(toggleController) { alert.runModal() }
                guard modalResult == .alertFirstButtonReturn else { return }
                let selectedBoundary = boundaryOptions.first(where: { $0.1 == boundary.titleOfSelectedItem })?.0 ?? "safe_boundary"
                let selectedStrategy = strategyOptions.first(where: { $0.1 == strategy.titleOfSelectedItem })?.0 ?? "adaptive"
                let isSavingWithMasterOn = master.state == .on
                let persistedReview = isSavingWithMasterOn ? (savedReviewState == .on) : (review.state == .on)
                let persistedDefer = isSavingWithMasterOn ? (savedDeferState == .on) : (deferToggle.state == .on)
                let persistedLegion = isSavingWithMasterOn ? (savedLegionState == .on) : (legion.state == .on)
                self.client.request("agentSwarm.settings", params: ["action": "set", "enabled": isSavingWithMasterOn, "criticalCompletionReview": persistedReview, "maxInitialReviews": Int(initialReviewLimit.titleOfSelectedItem ?? "1") ?? 1, "maxCorrectivePasses": Int(correctivePassLimit.titleOfSelectedItem ?? "1") ?? 1, "autoDeferOnCompletionReview": autoDeferReview.state == .on, "hierarchicalDefer": persistedDefer, "legion": persistedLegion, "deferBoundary": selectedBoundary, "strategy": selectedStrategy, "maxSpecialists": Int(specialistCap.titleOfSelectedItem ?? "4") ?? 4]) { [weak self] saved in
                    DispatchQueue.main.async {
                        guard let self else { return }
                        if case .success(let settings) = saved {
                            let master = settings["enabled"] as? Bool == true
                            self.criticalReviewEnabled = master || settings["criticalCompletionReview"] as? Bool == true
                            self.legionEnabled = master || settings["legion"] as? Bool == true
                            self.swarmDeferEnabled = master || settings["hierarchicalDefer"] as? Bool == true
                            self.maxSpecialists = settings["maxSpecialists"] as? Int ?? 4
                            if !self.swarmDeferEnabled { self.swarmDeferred = false }
                            self.updateStatus()
                            self.showNotice("Agent Swarm settings saved for this machine. CLI and the native app will use them.")
                        }
                        else { self.showNotice("Could not save Agent Swarm settings.") }
                    }
                }
            }
        }
    }
}

