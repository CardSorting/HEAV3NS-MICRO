// [LAYER: UI]
import AppKit
import Sparkle

@MainActor
final class ApplicationDelegate: NSObject, NSApplicationDelegate, NSMenuItemValidation {
    private var launcher: LauncherWindow?
    private var newProjectWindow: NewProjectWindow?
    private let projects = ProjectLibrary(persist: IntegrationProbe() == nil)
    private var workspaces = [String: WorkspaceWindow]()
    private var quitting = false
    private lazy var updates = NativeUpdater(readiness: { [weak self] completion in
        guard let self, !self.quitting else { completion(false); return }
        completion(self.workspaces.values.allSatisfy { $0.prepareForUpdateRestart() })
    }, waiting: {
        guard let window = NSApp.keyWindow, window.attachedSheet == nil else { return }
        let alert = NSAlert()
        alert.messageText = "Your update is ready when you are"
        alert.informativeText = "Finish active work and save your edits, then choose HEAV3NS-CODE → Install Downloaded Update… to restart. Your update will wait."
        alert.addButton(withTitle: "Keep Working")
        alert.beginSheetModal(for: window)
    })

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.regular)
        if let icon = BrandAssets.appIcon { NSApp.applicationIconImage = icon }
        if !ProcessInfo.processInfo.arguments.contains("--integration-probe") {
            let hasLaunched = UserDefaults.standard.bool(forKey: "hasLaunchedBefore")
            if !hasLaunched {
                UserDefaults.standard.set(true, forKey: "hasLaunchedBefore")
                SwarmRadarManager.shared.ping(action: "Fresh Cockpit Activated", source: "app_first_run")
            } else {
                SwarmRadarManager.shared.ping(action: "Cockpit Online", source: "app_launch")
            }
            SwarmRadarManager.shared.startPeriodicKeepAlive()
        }
        buildMenus()
        if !ProcessInfo.processInfo.arguments.contains("--integration-probe") { updates.start() }
        if let probe = IntegrationProbe() { for path in probe.projectPaths { projects.opened(URL(fileURLWithPath: path)) } }
        let arguments = ProcessInfo.processInfo.arguments
        let workspaceIndexes = arguments.indices.filter { arguments[$0] == "--workspace" && arguments.indices.contains($0 + 1) }
        if !workspaceIndexes.isEmpty {
            for index in workspaceIndexes { open(URL(fileURLWithPath: arguments[index + 1])) }
        } else { showLauncher() }
        NSApp.activate(ignoringOtherApps: true)
    }
    private func buildMenus() {
        let main = NSMenu()
        let applicationItem = NSMenuItem(); let applicationMenu = NSMenu()
        applicationMenu.addItem(withTitle: "About HEAV3NS-CODE", action: #selector(about), keyEquivalent: "").target = self
        if !ProcessInfo.processInfo.arguments.contains("--integration-probe") { updates.addMenuItems(to: applicationMenu) }
        let swarmRadarItem = applicationMenu.addItem(withTitle: "Sovereign Swarm Radar…", action: #selector(showSwarmRadar), keyEquivalent: "")
        swarmRadarItem.target = self
        applicationMenu.addItem(withTitle: "ChatGPT Accounts…", action: #selector(showChatGPTAccounts), keyEquivalent: "").target = self
        applicationMenu.addItem(.separator())
        applicationMenu.addItem(withTitle: "Hide HEAV3NS-CODE", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        applicationMenu.addItem(withTitle: "Quit HEAV3NS-CODE", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        applicationItem.submenu = applicationMenu; main.addItem(applicationItem)
        let fileItem = NSMenuItem(); fileItem.title = "File"; let file = NSMenu(title: "File")
        let newProject = file.addItem(withTitle: "New Project…", action: #selector(createProject), keyEquivalent: "n")
        newProject.keyEquivalentModifierMask = [.command, .shift]; newProject.target = self
        file.addItem(withTitle: "Open Workspace…", action: #selector(openFolder), keyEquivalent: "o").target = self
        file.addItem(withTitle: "Open File…", action: #selector(openFile), keyEquivalent: "p").target = self
        file.addItem(withTitle: "Go to Line…", action: #selector(showGoToLine), keyEquivalent: "g").target = self
        file.addItem(withTitle: "Project Library", action: #selector(showProjectLibrary), keyEquivalent: "0").target = self
        file.addItem(withTitle: "New Conversation", action: #selector(newConversation), keyEquivalent: "n").target = self
        file.addItem(withTitle: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        fileItem.submenu = file; main.addItem(fileItem)
        let editItem = NSMenuItem(); editItem.title = "Edit"; let edit = NSMenu(title: "Edit")
        edit.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        for (name, action, key) in [("Cut", #selector(NSText.cut(_:)), "x"), ("Copy", #selector(NSText.copy(_:)), "c"), ("Paste", #selector(NSText.paste(_:)), "v"), ("Select All", #selector(NSText.selectAll(_:)), "a")] { edit.addItem(withTitle: name, action: action, keyEquivalent: key) }
        edit.addItem(.separator())
        edit.addItem(withTitle: "Find in Current Page…", action: #selector(findInInspection), keyEquivalent: "f").target = self
        editItem.submenu = edit; main.addItem(editItem)
        let viewItem = NSMenuItem(); viewItem.title = "Workspace"; let view = NSMenu(title: "Workspace")
        let palette = view.addItem(withTitle: "Actions…", action: #selector(actions), keyEquivalent: "p"); palette.keyEquivalentModifierMask = [.command, .shift]; palette.target = self
        let commandPalette = view.addItem(withTitle: "Command Palette…", action: #selector(openCommandPalette), keyEquivalent: "k"); commandPalette.target = self
        let goToSymbol = view.addItem(withTitle: "Go to Symbol in File…", action: #selector(showGoToSymbol), keyEquivalent: "o"); goToSymbol.keyEquivalentModifierMask = [.command, .shift]; goToSymbol.target = self
        view.addItem(withTitle: "Toggle Navigation", action: #selector(toggleNavigation), keyEquivalent: "b").target = self
        let models = view.addItem(withTitle: "Model & Provider…", action: #selector(showModels), keyEquivalent: "m")
        models.keyEquivalentModifierMask = [.command, .shift]; models.target = self
        if let f3Scalar = UnicodeScalar(NSF3FunctionKey) {
            let fastModels = view.addItem(withTitle: "Fast Switch Model…", action: #selector(showModels), keyEquivalent: String(f3Scalar))
            fastModels.keyEquivalentModifierMask = []; fastModels.target = self
        }
        let stepPostureItem = view.addItem(withTitle: "Cycle Execution Posture", action: #selector(stepPosture), keyEquivalent: "")
        stepPostureItem.target = self
        let undoTurnItem = view.addItem(withTitle: "Undo Last Turn Edits", action: #selector(undoLastTurn), keyEquivalent: "z")
        undoTurnItem.keyEquivalentModifierMask = [.command, .option]; undoTurnItem.target = self
        let previewItem = view.addItem(withTitle: "Preview Outbound Request", action: #selector(showRequestPreview), keyEquivalent: "i")
        previewItem.keyEquivalentModifierMask = [.command, .shift]; previewItem.target = self
        view.addItem(withTitle: "Focus Composer", action: #selector(focusComposer), keyEquivalent: "l").target = self
        view.addItem(.separator())
        let chats = view.addItem(withTitle: "Chats", action: #selector(showChats), keyEquivalent: "c")
        chats.keyEquivalentModifierMask = [.command, .shift]; chats.target = self
        view.addItem(withTitle: "Code", action: #selector(showFiles), keyEquivalent: "1").target = self
        view.addItem(withTitle: "Changes", action: #selector(showChanges), keyEquivalent: "2").target = self
        let scItem = view.addItem(withTitle: "Source Control…", action: #selector(showSourceControl), keyEquivalent: "g")
        scItem.keyEquivalentModifierMask = [.command, .shift]; scItem.target = self
        view.addItem(withTitle: "Recorded Runs", action: #selector(showSessions), keyEquivalent: "3").target = self
        view.addItem(withTitle: "Walkthrough", action: #selector(showWalkthrough), keyEquivalent: "6").target = self
        view.addItem(withTitle: "Run Project", action: #selector(showProjectRun), keyEquivalent: "r").target = self
        view.addItem(withTitle: "Checkpoints", action: #selector(showCheckpoints), keyEquivalent: "7").target = self
        view.addItem(withTitle: "Conversation Memory…", action: #selector(showMemory), keyEquivalent: "8").target = self
        view.addItem(withTitle: "Conversation Context", action: #selector(showContext), keyEquivalent: "9").target = self
        let codeCheckItem = view.addItem(withTitle: "Check Code", action: #selector(showCodeCheck), keyEquivalent: "k")
        codeCheckItem.keyEquivalentModifierMask = [.command, .shift]; codeCheckItem.target = self
        view.addItem(withTitle: "Saved State", action: #selector(showSavedState), keyEquivalent: "").target = self
        view.addItem(withTitle: "Up Next", action: #selector(showQueue), keyEquivalent: "4").target = self
        view.addItem(withTitle: "Review Request…", action: #selector(showApprovalReview), keyEquivalent: "5").target = self
        let allowItem = view.addItem(withTitle: "Allow Operation Once", action: #selector(allowOperation), keyEquivalent: "a")
        allowItem.keyEquivalentModifierMask = [.command, .shift]; allowItem.target = self
        let denyItem = view.addItem(withTitle: "Deny Operation", action: #selector(denyOperation), keyEquivalent: "d")
        denyItem.keyEquivalentModifierMask = [.command, .shift]; denyItem.target = self
        view.addItem(.separator())
        view.addItem(withTitle: "Engage Specialist Team…", action: #selector(engageSpecialists), keyEquivalent: "").target = self
        view.addItem(withTitle: "Formation Status…", action: #selector(showFormationStatus), keyEquivalent: "").target = self
        view.addItem(withTitle: "Formation History…", action: #selector(showFormationHistory), keyEquivalent: "").target = self
        view.addItem(withTitle: "Inspect Latest Specialist Stream…", action: #selector(showLatestSpecialistStream), keyEquivalent: "").target = self
        view.addItem(withTitle: "Review Latest Completion…", action: #selector(requestCriticalReview), keyEquivalent: "").target = self
        view.addItem(withTitle: "Defer Agent Work", action: #selector(toggleAgentDefer), keyEquivalent: "").target = self
        view.addItem(withTitle: "Command Recovery…", action: #selector(showCommandRecovery), keyEquivalent: "").target = self
        view.addItem(withTitle: "Agent Swarm Settings…", action: #selector(showAgentSwarmSettings), keyEquivalent: "").target = self
        let toggleRadar = view.addItem(withTitle: "Toggle Swarm Radar", action: #selector(toggleSwarmRadar), keyEquivalent: "r")
        toggleRadar.keyEquivalentModifierMask = [.command, .option]
        toggleRadar.target = self
        view.addItem(.separator())
        view.addItem(withTitle: "Back in Inspector", action: #selector(inspectBack), keyEquivalent: "[").target = self
        view.addItem(withTitle: "Forward in Inspector", action: #selector(inspectForward), keyEquivalent: "]").target = self
        let maxContext = view.addItem(withTitle: "Toggle Maximize Task Context", action: #selector(toggleMaximizeContext), keyEquivalent: "m")
        maxContext.keyEquivalentModifierMask = [.command, .control]; maxContext.target = self
        view.addItem(.separator())
        view.addItem(withTitle: "Stop LUMI", action: #selector(stop), keyEquivalent: ".").target = self
        viewItem.submenu = view; main.addItem(viewItem)
        let windowItem = NSMenuItem(); windowItem.title = "Window"; let window = NSMenu(title: "Window")
        window.addItem(withTitle: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        let windowRadar = window.addItem(withTitle: "Sovereign Swarm Radar", action: #selector(showSwarmRadar), keyEquivalent: "")
        windowRadar.target = self
        windowItem.submenu = window; main.addItem(windowItem); NSApp.windowsMenu = window
        NSApp.mainMenu = main
    }
    @objc private func showSwarmRadar() {
        SwarmRadarWindow.shared.present()
    }
    @objc private func toggleSwarmRadar() {
        SwarmRadarWindow.shared.toggle()
    }
    private var current: WorkspaceWindow? { workspaces.values.first { $0.window === NSApp.keyWindow || $0.window === NSApp.keyWindow?.sheetParent || $0.window === NSApp.keyWindow?.parent } }
    @objc private func about() {
        NSApp.orderFrontStandardAboutPanel(options: [.applicationName: "HEAV3NS-CODE", .applicationVersion: Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "Development", .credits: NSAttributedString(string: "A native workspace for LUMI.\nExecution and credentials remain in the CLI.")])
    }
    @objc func openFolder() {
        let panel = NSOpenPanel(); panel.title = "Open a Workspace"; panel.canChooseFiles = false; panel.canChooseDirectories = true; panel.allowsMultipleSelection = false; panel.prompt = "Open Workspace"
        if panel.runModal() == .OK, let url = panel.url { open(url) }
    }
    @objc func createProject() {
        if let sheet = newProjectWindow?.window { sheet.makeKeyAndOrderFront(nil); return }
        if NSApp.keyWindow == nil { showLauncher() }
        guard let parent = NSApp.keyWindow?.sheetParent ?? NSApp.keyWindow, parent.attachedSheet == nil else { return }
        let controller = NewProjectWindow(defaults: ProcessInfo.processInfo.arguments.contains("--integration-probe") ? nil : .standard) { [weak self] url in self?.open(url) }
        controller.onClose = { [weak self] in self?.newProjectWindow = nil }
        newProjectWindow = controller; controller.present(on: parent)
    }
    private func open(_ url: URL, activate: Bool = true, unavailable: ((String) -> Void)? = nil) {
        let canonical = url.resolvingSymlinksInPath().standardizedFileURL
        var isDirectory: ObjCBool = false
        guard canonical.isFileURL, FileManager.default.fileExists(atPath: canonical.path, isDirectory: &isDirectory), isDirectory.boolValue else {
            if let unavailable { unavailable(canonical.path); return }
            let alert = NSAlert(); alert.messageText = "This workspace folder is unavailable."
            alert.informativeText = "It may have moved or been removed. Choose its current location to open it.\n\n\(canonical.path)"
            alert.addButton(withTitle: "Choose Folder…"); alert.addButton(withTitle: "Cancel")
            if alert.runModal() == .alertFirstButtonReturn { openFolder() }
            return
        }
        if let existing = workspaces[canonical.path] {
            if !ProcessInfo.processInfo.arguments.contains("--integration-probe") { projects.opened(canonical) }
            if activate { existing.showWindow(nil); existing.window?.makeKeyAndOrderFront(nil) }; return
        }
        let workspace = WorkspaceWindow(folder: canonical)
        workspace.onProjects = { [weak self] in self?.showLauncher() }
        workspace.onAccount = { [weak self] in self?.showLauncher(account: true) }
        workspaces[canonical.path] = workspace
        workspace.onClosed = { [weak self, weak workspace] in
            guard let self, self.workspaces[canonical.path] === workspace else { return }
            self.workspaces.removeValue(forKey: canonical.path)
            if self.quitting && self.workspaces.isEmpty { NSApp.reply(toApplicationShouldTerminate: true) }
            else if self.workspaces.isEmpty { self.showLauncher() }
        }
        launcher?.suspend()
        if activate { workspace.showWindow(nil); workspace.window?.makeKeyAndOrderFront(nil) }
        workspace.attach()
        if !ProcessInfo.processInfo.arguments.contains("--integration-probe") {
            projects.opened(canonical)
            var recent = UserDefaults.standard.stringArray(forKey: "recentWorkspaces") ?? []
            recent.removeAll { $0 == canonical.path }; recent.insert(canonical.path, at: 0)
            UserDefaults.standard.set(Array(recent.prefix(12)), forKey: "recentWorkspaces")
        }
    }
    private func showLauncher(account: Bool = false) {
        if launcher == nil {
            if let probe = IntegrationProbe(), probe.launcherPass { for path in probe.projectPaths { projects.opened(URL(fileURLWithPath: path)) }; if let first = projects.projects.first { projects.togglePin(first.path) } }
            let controller = LauncherWindow(library: projects)
            controller.onOpenFolder = { [weak self] in self?.openFolder() }
            controller.onCreateProject = { [weak self] in self?.createProject() }
            controller.onOpenProject = { [weak self] url in self?.open(url) }
            controller.canChangeAccount = { [weak self] in self?.workspaces.isEmpty == true }
            launcher = controller
        }
        launcher?.present(account: account)
    }
    @objc private func showProjectLibrary() { showLauncher() }
    @objc private func newConversation() { current?.newConversation() }
    @objc private func showModels() { current?.showModels() }
    @objc private func showChatGPTAccounts() { ChatGPTAccountsWindow.shared.present() }
    @objc private func openFile() { current?.showQuickOpen() }
    @objc private func showGoToLine() { current?.showGoToLine() }
    @objc private func showGoToSymbol() { current?.showGoToSymbol() }
    @objc private func showWalkthrough() { current?.showWalkthrough() }
    @objc private func showProjectRun() { current?.showProjectRun() }
    @objc private func showCheckpoints() { current?.showCheckpoints() }
    @objc private func showMemory() { current?.showMemory() }
    @objc private func showContext() { current?.showContext() }
    @objc private func showCodeCheck() { current?.showCodeCheck() }
    @objc private func showSavedState() { current?.showSavedState() }
    @objc private func toggleNavigation() { current?.toggleNavigation() }
    @objc private func focusComposer() { current?.focusComposer() }
    @objc private func showChats() { current?.showChats() }
    @objc private func showFiles() { current?.showFiles() }
    @objc private func showChanges() { current?.showChanges() }
    @objc private func showSourceControl() { current?.showSourceControl() }
    @objc private func showSessions() { current?.showSessions() }
    @objc private func showQueue() { current?.showQueue() }
    @objc private func showApprovalReview() { current?.showApprovalReview() }
    @objc private func allowOperation() { current?.allowOnce() }
    @objc private func denyOperation() { current?.deny() }
    @objc private func inspectBack() { current?.inspectBack() }
    @objc private func inspectForward() { current?.inspectForward() }
    @objc private func toggleMaximizeContext() { current?.toggleMaximizeContext() }
    @objc private func engageSpecialists() { current?.engageSpecialists() }
    @objc private func showFormationStatus() { current?.showFormationStatus() }
    @objc private func showFormationHistory() { current?.showFormationHistory() }
    @objc private func showLatestSpecialistStream() { current?.showLatestSpecialistStream() }
    @objc private func requestCriticalReview() { current?.requestCriticalReview() }
    @objc private func toggleAgentDefer() { current?.toggleAgentDefer() }
    @objc private func showCommandRecovery() { current?.showCommandRecovery() }
    @objc private func showAgentSwarmSettings() { current?.showAgentSwarmSettings() }
    @objc private func findInInspection() {
        if let launcher, NSApp.keyWindow === launcher.window { launcher.focusSearch() }
        else if current?.isMemoryKeyWindow == true { current?.findMemory() }
        else { current?.findInInspection() }
    }
    @objc private func stop() { current?.stop() }
    @objc private func stepPosture() { current?.stepPosture() }
    @objc private func undoLastTurn() { current?.undoLastTurn() }
    @objc private func showRequestPreview() { current?.showRequestPreview() }
    @objc private func openCommandPalette() {
        if let current { current.showCommandPalette() } else { openFolder() }
    }
    @objc private func actions() {
        if let current { current.showActions() } else { openFolder() }
    }
    func validateMenuItem(_ menuItem: NSMenuItem) -> Bool {
        if menuItem.action == #selector(openFolder) {
            menuItem.title = "Open Workspace…"
            return true
        }
        if menuItem.action == #selector(showMemory) { return current?.canShowMemory == true }
        if menuItem.action == #selector(showContext) { return current?.canShowContext == true }
        if menuItem.action == #selector(showCodeCheck) { return current?.canCheckCode == true }
        if menuItem.action == #selector(showSavedState) { return current?.canShowSavedState == true }
        if menuItem.action == #selector(showModels) { return current?.canChooseModels == true }
        if menuItem.action == #selector(showProjectRun) { return current?.client.capabilities.contains("project-run-v1") == true }
        if menuItem.action == #selector(showCheckpoints) { return current?.client.capabilities.contains("checkpoints-v1") == true }
        if menuItem.action == #selector(openFile) { return current?.canOpenFile == true }
        if [#selector(showGoToLine), #selector(showGoToSymbol)].contains(menuItem.action) { return current?.canOpenFile == true }
        if menuItem.action == #selector(newConversation) { return current?.canCreateConversation == true }
        if menuItem.action == #selector(inspectBack) { return current?.canInspectBack == true }
        if menuItem.action == #selector(inspectForward) { return current?.canInspectForward == true }
        if menuItem.action == #selector(engageSpecialists) { return current?.canEngageSpecialists == true }
        if menuItem.action == #selector(requestCriticalReview) { return current?.canRequestCriticalReview == true }
        if menuItem.action == #selector(toggleAgentDefer) {
            menuItem.title = current?.isSwarmDeferred == true ? "Resume Agent Work" : "Defer Agent Work"
            return current?.canToggleAgentDefer == true
        }
        if [#selector(showFormationStatus), #selector(showFormationHistory), #selector(showLatestSpecialistStream), #selector(showCommandRecovery), #selector(showAgentSwarmSettings)].contains(menuItem.action) {
            return current?.canAccessAgentSwarm == true
        }
        if menuItem.action == #selector(findInInspection) {
            if current?.isMemoryKeyWindow == true { menuItem.title = "Find Conversation Memory…"; return current?.canFindMemory == true }
            let inLibrary = launcher != nil && NSApp.keyWindow === launcher?.window
            if inLibrary { menuItem.title = "Find Projects…"; return launcher?.canFindProjects == true }
            if current?.canFindInInspection == true { menuItem.title = "Find in Current Page…"; return true }
            if current?.canFindInWalkthrough == true { menuItem.title = "Filter Walkthrough Activity…"; return true }
            menuItem.title = "Find in Current Page…"
            return false
        }
        if menuItem.action == #selector(stop) { return current?.hasWork == true }
        if [#selector(allowOperation), #selector(denyOperation)].contains(menuItem.action) { return current?.canDecideApproval == true }
        if menuItem.action == #selector(showApprovalReview) { return current?.canReviewApproval == true }
        if [#selector(toggleNavigation), #selector(focusComposer), #selector(showChats), #selector(showFiles), #selector(showChanges), #selector(showSourceControl), #selector(showSessions), #selector(showQueue), #selector(showWalkthrough), #selector(toggleMaximizeContext), #selector(stepPosture)].contains(menuItem.action) { return current != nil }
        if menuItem.action == #selector(undoLastTurn) { return current?.client.isReady == true && current?.hasWork == false }
        if menuItem.action == #selector(showRequestPreview) { return current?.client.isReady == true }
        return true
    }
    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        if workspaces.isEmpty { return .terminateNow }
        for workspace in workspaces.values where !workspace.prepareToClose() {
            workspace.showWindow(nil); workspace.window?.makeKeyAndOrderFront(nil)
            return .terminateCancel
        }
        let activeCount = workspaces.values.filter { $0.hasWork }.count
        if activeCount > 0 {
            let alert = NSAlert(); alert.messageText = "Stop work in \(activeCount) workspace\(activeCount == 1 ? "" : "s") and quit?"
            alert.informativeText = "LUMI will cancel active work and flush recorded state. Existing edits are retained."
            alert.addButton(withTitle: "Keep Running"); alert.addButton(withTitle: "Stop and Quit")
            if alert.runModal() != .alertSecondButtonReturn { return .terminateCancel }
        }
        quitting = true
        for workspace in Array(workspaces.values) { workspace.stopAndClose() }
        return workspaces.isEmpty ? .terminateNow : .terminateLater
    }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool { if !flag { showLauncher() }; return true }
    func application(_ application: NSApplication, open urls: [URL]) { for url in urls { open(url) } }
}

@main struct Heav3nsCodeApp {
    @MainActor static func main() {
        let delegate = ApplicationDelegate()
        NSApplication.shared.delegate = delegate
        NSApplication.shared.run()
        withExtendedLifetime(delegate) {}
    }
}
