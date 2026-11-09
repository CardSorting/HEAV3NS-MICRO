// [LAYER: UI]
import AppKit
import CryptoKit
import Foundation

final class WorkspaceWindow: NSWindowController, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate, NSTextViewDelegate, NSToolbarDelegate {
    let folder: URL
    var client = RuntimeClient()
    var onClosed: (() -> Void)?
    var onProjects: (() -> Void)?
    var onAccount: (() -> Void)?
    let status = NSTextField(wrappingLabelWithString: "Preparing workspace…")
    let stopButton = NSButton(title: "Stop", target: nil, action: nil)
    let sendButton = NSButton(title: "Send", target: nil, action: nil)
    let queueButton = NSButton(title: "Up Next", target: nil, action: nil)
    let queueContent = QueuedInstructions()
    let queuePopover = NSPopover()
    var queueEditor: QueuedInstructionEditor?
    var editingQueued: QueuedInstruction?
    var removingQueued = Set<String>()
    var swarmDeferred = false
    var swarmDeferEnabled = false
    var criticalReviewEnabled = false
    var legionEnabled = false
    var maxSpecialists = 4
    let jumpButton = NSButton(title: "Jump to Latest", target: nil, action: nil)
    let composer = ComposerView()
    let attachmentBar = AttachmentTokenBar()
    let jumpBar = NSPathControl()
    let toolbarHistory = NSSegmentedControl(images: [
        NSImage(systemSymbolName: "chevron.left", accessibilityDescription: "Back in history")!,
        NSImage(systemSymbolName: "chevron.right", accessibilityDescription: "Forward in history")!
    ], trackingMode: .momentary, target: nil, action: nil)
    let transcript = NSTableView()
    let transcriptScroll = TranscriptScrollView()
    let turnScrubber = TurnScrubberRail()
    let inspection = NSTextView()
    let inspectionScroll = NSScrollView()
    let documentView = NativeDocumentView()
    var binaryDocumentLoader: BinaryDocumentLoader?
    let viewportStore: WorkspaceViewportStore
    var currentViewport: WorkspaceViewport?
    var viewportBranch: String?
    var viewportObserver: NSObjectProtocol?
    var viewportSaveTimer: Timer?
    var viewportRestoring = false
    var viewportSaveFailed = false
    var viewportBranchGeneration = 0
    var viewportBranchTimer: Timer?
    var viewportBranchCheckPending = false
    var viewportBranchCheckFailureShown = false
    let inspector = NSStackView()
    let inspectorTitle = NSTextField(labelWithString: "Inspect")
    let navigation = NavigationSidebar()
    lazy var conversations = ConversationSidebar(folder: folder)
    let leftSidebar = NSVisualEffectView()
    let navigatorTabs = NSSegmentedControl(labels: ["Chats", "Files", "Changes"], trackingMode: .selectOne, target: nil, action: nil)
    let leftNavigatorStack = NSStackView()
    let chatTitle = JourneyStyle.label("New conversation", size: 21, weight: .semibold)
    let chatState = JourneyStyle.label("Tell LUMI what you want to build.", size: 12, secondary: true)
    let contextPanel = NSStackView()
    let contextTabs = NSSegmentedControl(labels: ["Walkthrough", "Changes", "Inspect", "Run"], trackingMode: .selectOne, target: nil, action: nil)
    let projectRun = ProjectRunView()
    let projectRunScroll = NSScrollView()
    var checkpointWindow: CheckpointWindow?
    let checkpointsButton = NSButton(title: "Checkpoints", target: nil, action: nil)
    let walkthrough = WalkthroughView()
    var walkthroughLoaded: String?
    var walkthroughLoadGeneration = 0
    var workspaceRefreshTimer: Timer?
    let outline = NSPopUpButton()
    var declarations = [CodeDeclaration]()
    var contextWidth: NSLayoutConstraint?
    var browserHeight: NSLayoutConstraint?
    let contextSeparator = JourneyStyle.rule()
    let browseContext = NSButton(title: "Browse files", target: nil, action: nil)
    var browsingCompact = true
    var isContextMaximized = false
    var currentConversation: String?
    var conversationList = [ConversationSummary]()
    var conversationOffset = 0
    var moreConversations = false
    var switchingConversation = false
    var expectedConversation: String?
    var conversationGeneration = 0
    var draftByConversation = [String: String]()
    let approval = NSStackView()
    let approvalText = NSTextView()
    let reviewApprovalButton = NSButton(title: "Review Request…", target: nil, action: nil)
    let denyButton = NSButton(title: "Deny", target: nil, action: nil); let allowButton = NSButton(title: "Allow Once", target: nil, action: nil)
    var pendingLineJump: (line: Int, column: Int)?
    var approvalReview: ApprovalReview?
    struct ApprovalDecision: Equatable {
        let id = UUID()
        let request: ApprovalRequest
        let context: UUID
    }
    var decidingApproval: ApprovalDecision?
    var approvalFailureAttempt: ApprovalDecision?
    var approvalReviewContext: UUID?
    var podContextID = UUID()
    var approvalFailure: String?
    let surfaces = NSSegmentedControl(labels: ["Conversation", "Task context"], trackingMode: .selectOne, target: nil, action: nil)
    var rows = [TranscriptRow]()
    var runs = [[String: Any]]()
    struct SideQuestion {
        let id: String, conversation: String, question: String
        var answer: String? = nil
        var failed = false
        var contextLimited = false
    }
    var sideQuestions = [SideQuestion]()
    var sideQuestionPending: String?
    var projectTaskRevision = -1
    var projectTaskDisplayedRevision = -1
    var projectTaskMovePending = false
    var projectTaskOffset = 0
    var projectTaskPreviousOffsets = [Int]()
    var projectTaskFilters = [ProjectTaskFilter]()
    var projectTasksLoading = false
    var projectTasksRefreshPending = false
    var sequence = 0
    var active: String?
    var pendingRequest: String?
    var currentApproval: ApprovalRequest?
    var forceAvailable = false
    var closing = false
    var stopped = false
    var showingInspector = true
    var connection = "Preparing"
    var directory = "."
    var nextChunk: (() -> Void)?
    var previousChunk: (() -> Void)?
    var reloadInspection: (() -> Void)?
    var inspectionHistory = InspectionHistory()
    let inspectionBack = NSButton()
    let inspectionForward = NSButton()
    let inspectionFind = NSButton()
    let recordedOutput = NSButton(title: "View Recorded Output", target: nil, action: nil)
    let checkCode = NSButton(title: "Check Code", target: nil, action: nil)
    let prepareCodeReview = NSButton(title: "Prepare Review", target: nil, action: nil)
    var spiderDetails: SpiderCheckDetails?
    var savedStateDetails: SavedStateDetails?
    let relatedEvidence = NSPopUpButton(frame: .zero, pullsDown: true)
    var memoryWindow: ConversationMemoryWindow?
    var evidenceLinks = [RunDetails.Related]()
    var contextLinks = [ConversationContextLink]()
    var showingContextSummary = false
    let previousButton = NSButton(title: "Previous", target: nil, action: nil)
    let pageLabel = NSTextField(labelWithString: "Read only")
    let breadcrumb = NSPathControl()
    let diffScope = NSSegmentedControl(labels: ["Working Tree", "Staged"], trackingMode: .selectOne, target: nil, action: nil)
    let revealButton = NSButton(title: "Reveal in Finder", target: nil, action: nil)
    let reconnectButton = NSButton(title: "Reconnect", target: nil, action: nil)
    let stopSendButton = NSButton(title: "Stop & Send", target: nil, action: nil)
    let notice = NSTextField(wrappingLabelWithString: "")
    let noticeBar = NSStackView()
    let modelButton = NSButton(title: "Model & Provider…", target: nil, action: nil)
    var modelPicker: ModelPicker?
    let reasoningPicker = ReasoningPicker()
    let hint = NSTextField(labelWithString: "Enter to send · Shift–Enter for a new line")
    var sidebarPreferred = true
    var compactSidebarRevealed = false
    var navigationGeneration = 0
    var inspectionGeneration = 0
    var navigationOffset = 0
    var navigationQuery = ""
    var nextNavigationPage: (() -> Void)?
    var previousNavigationPage: (() -> Void)?
    var navigationData = [SidebarEntry]()
    var inspectedFile: String?
    var inspectedDiff: String?
    var expandedActions = Set<String>()
    var uncertainSubmission: WorkspaceDraft.Submission?
    var checkingSubmission = false
    let draftStore: WorkspaceDraftStore
    var recoverySubmission: WorkspaceDraft.Submission?
    var draftTimer: Timer?
    var savedDraftText = ""
    var savedDraftAttachments: [String] = []
    var savedSubmission: WorkspaceDraft.Submission?
    var draftSaveFailed = false
    var stopThenSend: (text: String, runs: Set<String>)?
    var elapsedTimer: Timer?
    var palette: ActionPalette?
    var quickOpen: QuickOpenWindow?
    let openFileButton = NSButton(title: "Open File…", target: nil, action: nil)
    var needsRebuild = false
    var cancelRequested = false
    var stopAcknowledged = true
    var cancellationToken: UUID?
    var recovering = false
    let terminalStates: Set<String> = ["completed", "failed", "cancelled", "timed_out", "interrupted"]
    let moreButton = NSButton(title: "Next Page", target: nil, action: nil)
    var layoutTimer: Timer?
    var resizeAnchor: (following: Bool, origin: NSPoint)?
    let probe = IntegrationProbe()

    var canMaximizeContext: Bool { showingInspector && !contextPanel.isHidden }
    var hasWork: Bool { runs.contains { !terminalStates.contains($0["state"] as? String ?? "") } || pendingRequest != nil }
    var isClosing: Bool { closing }
    var canInspectBack: Bool { inspectionHistory.canGoBack && window?.attachedSheet == nil }
    var canInspectForward: Bool { inspectionHistory.canGoForward && window?.attachedSheet == nil }
    var canFindInInspection: Bool { showingInspector && !inspector.isHidden && documentView.isHidden && window?.attachedSheet == nil }
    var canFindInWalkthrough: Bool { showingInspector && !contextPanel.isHidden && walkthrough.canFocusSearch && window?.attachedSheet == nil }
    var canReviewApproval: Bool { currentApproval != nil && window?.attachedSheet == nil && !closing }
    var canOpenFile: Bool { client.isReady && client.capabilities.contains("file-search-v1") && !switchingConversation && !closing && window?.attachedSheet == nil }
    var canChooseModels: Bool { !reasoningPicker.pending && client.isReady && client.capabilities.contains("model-selection-v1") && !closing && window?.attachedSheet == nil }
    var canCreateConversation: Bool { canSwitchConversation && window?.attachedSheet == nil }
    var canAccessAgentSwarm: Bool { client.isReady && !closing && window?.attachedSheet == nil }
    var canEngageSpecialists: Bool { canAccessAgentSwarm && legionEnabled }
    var canToggleAgentDefer: Bool { canAccessAgentSwarm && (swarmDeferEnabled || swarmDeferred) }
    var canRequestCriticalReview: Bool { canAccessAgentSwarm && criticalReviewEnabled }
    var isSwarmDeferred: Bool { swarmDeferred }
    var canSwitchConversation: Bool { client.isReady && client.capabilities.contains("conversations") && !hasWork && !switchingConversation && uncertainSubmission == nil && !closing }

    @objc func showProjectRun() { contextTabs.selectedSegment = 3; showingInspector = true; surfaces.selectedSegment = 1; switchSurface() }
    @objc func showCheckpoints() { openCheckpoints(selected: nil) }

    init(folder: URL, runtimeClient: RuntimeClient? = nil, draftStore: WorkspaceDraftStore? = nil, viewportStore: WorkspaceViewportStore? = nil) {
        self.folder = folder
        self.draftStore = draftStore ?? WorkspaceDraftStore(folder: folder, root: IntegrationProbe()?.draftRoot)
        self.viewportStore = viewportStore ?? WorkspaceViewportStore(folder: folder)
        if let runtimeClient { client = runtimeClient }
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1440, height: 880), styleMask: [.titled, .closable, .miniaturizable, .resizable], backing: .buffered, defer: false)
        window.title = "\(folder.lastPathComponent) — HEAV3NS-CODE"
        window.subtitle = folder.path
        window.minSize = NSSize(width: 720, height: 480)
        window.isReleasedWhenClosed = false
        if !ProcessInfo.processInfo.arguments.contains("--integration-probe") { window.setFrameAutosaveName("workspace-\(folder.path)") }
        super.init(window: window)
        window.delegate = self
        build()
        restoreDraft()
        configureClient()
        viewportBranchTimer = Timer.scheduledTimer(withTimeInterval: 10, repeats: true) { [weak self] _ in
            guard let self, self.window?.isVisible == true, self.window?.isMiniaturized == false, !self.closing else { return }
            self.checkViewportBranch()
        }
        window.center()
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    deinit {
        dispose()
    }

    /// Releases timers, observers, and auxiliary windows to ensure clean termination (`SPI-110`).
    func dispose() {
        viewportBranchTimer?.invalidate(); viewportBranchTimer = nil
        viewportSaveTimer?.invalidate(); viewportSaveTimer = nil
        workspaceRefreshTimer?.invalidate(); workspaceRefreshTimer = nil
        draftTimer?.invalidate(); draftTimer = nil
        elapsedTimer?.invalidate(); elapsedTimer = nil
        layoutTimer?.invalidate(); layoutTimer = nil
        if let viewportObserver { NotificationCenter.default.removeObserver(viewportObserver); self.viewportObserver = nil }
        binaryDocumentLoader?.cancel(); binaryDocumentLoader = nil
        memoryWindow?.dismiss(); memoryWindow = nil
        palette?.cancelOperation(nil); palette = nil
        modelPicker?.dismiss(restoreFocus: false); modelPicker = nil
        checkpointWindow?.close(); checkpointWindow = nil
    }

    func button(_ title: String, _ action: Selector, symbol: String? = nil) -> NSButton {
        let b = NSButton(title: title, target: self, action: action)
        b.bezelStyle = .rounded
        b.setContentHuggingPriority(.required, for: .horizontal)
        if let symbol { b.image = NSImage(systemSymbolName: symbol, accessibilityDescription: title); b.imagePosition = .imageLeading }
        return b
    }

    func stack(_ views: [NSView], vertical: Bool = false, spacing: CGFloat = 8) -> NSStackView {
        let s = NSStackView(views: views); s.orientation = vertical ? .vertical : .horizontal
        s.alignment = vertical ? .leading : .centerY; s.spacing = spacing; s.distribution = .fill; s.translatesAutoresizingMaskIntoConstraints = false
        return s
    }

    private func build() {
        guard let content = window?.contentView else { return }
        status.font = .systemFont(ofSize: 11); status.textColor = .secondaryLabelColor
        status.maximumNumberOfLines = 1; status.lineBreakMode = .byTruncatingTail
        stopButton.target = self; stopButton.action = #selector(stop); stopButton.bezelStyle = .texturedRounded
        stopButton.image = NSImage(systemSymbolName: "stop.fill", accessibilityDescription: "Stop All Work")
        stopButton.imagePosition = .imageLeading; stopButton.isEnabled = false; stopButton.setAccessibilityLabel("Stop All Work")
        stopButton.toolTip = "Stop current and queued instructions (⌘.)"
        let toolbar = NSToolbar(identifier: "workspace"); toolbar.delegate = self; toolbar.displayMode = .iconOnly
        toolbar.allowsUserCustomization = false
        toolbarHistory.target = self; toolbarHistory.action = #selector(toolbarHistoryClicked)
        toolbarHistory.segmentDistribution = .fit
        toolbarHistory.setAccessibilityLabel("Navigation History")
        toolbarHistory.setEnabled(false, forSegment: 0)
        toolbarHistory.setEnabled(false, forSegment: 1)
        toolbarHistory.frame = NSRect(x: 0, y: 0, width: 54, height: 24)
        toolbarHistory.widthAnchor.constraint(equalToConstant: 54).isActive = true
        jumpBar.pathStyle = .standard; jumpBar.isEditable = false; jumpBar.target = self; jumpBar.action = #selector(openJumpBarSegment)
        jumpBar.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        jumpBar.setAccessibilityLabel("Workspace jump bar")
        jumpBar.frame = NSRect(x: 0, y: 0, width: 220, height: 24)
        jumpBar.widthAnchor.constraint(greaterThanOrEqualToConstant: 120).isActive = true
        jumpBar.widthAnchor.constraint(lessThanOrEqualToConstant: 320).isActive = true
        jumpBar.heightAnchor.constraint(equalToConstant: 24).isActive = true
        updateJumpBar()
        window?.toolbar = toolbar; window?.toolbarStyle = .unified
        navigation.useAsContextBrowser()
        navigatorTabs.target = self; navigatorTabs.action = #selector(switchNavigatorTab); navigatorTabs.selectedSegment = 0
        navigatorTabs.setAccessibilityLabel("Navigator: Chats, Files, or Changes")
        navigatorTabs.segmentDistribution = .fillEqually
        leftSidebar.material = .sidebar; leftSidebar.blendingMode = .withinWindow; leftSidebar.state = .followsWindowActiveState
        leftSidebar.widthAnchor.constraint(equalToConstant: 250).isActive = true
        leftNavigatorStack.orientation = .vertical; leftNavigatorStack.alignment = .leading; leftNavigatorStack.spacing = 8
        leftNavigatorStack.edgeInsets = NSEdgeInsets(top: 14, left: 8, bottom: 14, right: 8)
        leftSidebar.addSubview(leftNavigatorStack)
        leftNavigatorStack.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            leftNavigatorStack.leadingAnchor.constraint(equalTo: leftSidebar.leadingAnchor),
            leftNavigatorStack.trailingAnchor.constraint(equalTo: leftSidebar.trailingAnchor),
            leftNavigatorStack.topAnchor.constraint(equalTo: leftSidebar.topAnchor),
            leftNavigatorStack.bottomAnchor.constraint(equalTo: leftSidebar.bottomAnchor)
        ])
        leftNavigatorStack.addArrangedSubview(navigatorTabs)
        navigatorTabs.widthAnchor.constraint(equalTo: leftNavigatorStack.widthAnchor, constant: -16).isActive = true
        leftNavigatorStack.addArrangedSubview(conversations)
        leftNavigatorStack.addArrangedSubview(navigation)
        conversations.widthAnchor.constraint(equalTo: leftNavigatorStack.widthAnchor).isActive = true
        navigation.widthAnchor.constraint(equalTo: leftNavigatorStack.widthAnchor).isActive = true
        navigation.isHidden = true
        conversations.onDelete = { [weak self] id in self?.confirmDeleteConversation(id) }
        conversations.onNew = { [weak self] in self?.newConversation() }
        conversations.onSelect = { [weak self] id in self?.selectConversation(id) }
        conversations.onProjects = { [weak self] in self?.onProjects?() }
        conversations.onAccount = { [weak self] in self?.onAccount?() }
        conversations.onMore = { [weak self] in self?.loadConversations(next: true) }
        conversations.onRecent = { [weak self] in self?.loadConversations() }
        navigation.onDestination = { [weak self] destination in self?.navigate(destination) }
        navigation.onOpen = { [weak self] entry in self?.openEntry(entry) }
        navigation.onBack = { [weak self] in self?.parentFolder() }
        navigation.onNext = { [weak self] in self?.nextNavigationPage?() }
        navigation.onPrevious = { [weak self] in self?.previousNavigationPage?() }
        navigation.onRefresh = { [weak self] in self?.refreshNavigation() }
        navigation.onFilter = { [weak self] query in
            guard let self else { return }; self.navigationQuery = query
            if self.navigation.destination == .files { self.loadDirectory(self.directory) }
            else { self.renderNavigation() }
        }

        let column = NSTableColumn(identifier: NSUserInterfaceItemIdentifier("transcript"))
        transcript.addTableColumn(column); transcript.headerView = nil; transcript.dataSource = self; transcript.delegate = self
        transcript.backgroundColor = .textBackgroundColor; transcript.selectionHighlightStyle = .none
        transcript.intercellSpacing = NSSize(width: 0, height: 20); transcript.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        transcript.setAccessibilityLabel("Agent conversation and tool activity")
        transcriptScroll.documentView = transcript; transcriptScroll.hasVerticalScroller = true
        transcriptScroll.beforeResize = { [weak self] following, origin in
            guard let self, self.resizeAnchor == nil else { return }
            self.resizeAnchor = (following, origin)
            DispatchQueue.main.async { [weak self] in self?.restoreTranscriptAfterResize() }
        }
        jumpButton.target = self; jumpButton.action = #selector(jumpLatest); jumpButton.isHidden = true
        jumpButton.bezelStyle = .rounded

        inspector.orientation = .vertical; inspector.alignment = .leading; inspector.spacing = 10
        inspectorTitle.font = .systemFont(ofSize: 13, weight: .semibold)
        inspectorTitle.lineBreakMode = .byTruncatingMiddle
        inspectorTitle.setContentHuggingPriority(.init(100), for: .horizontal)
        inspectorTitle.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        let refreshInspect = button("", #selector(refreshInspection), symbol: "arrow.clockwise")
        refreshInspect.imagePosition = .imageOnly
        refreshInspect.setAccessibilityLabel("Reload inspected content"); refreshInspect.toolTip = "Reload inspected content"
        for (control, symbol, label, action) in [
            (inspectionBack, "chevron.left", "Back (⌘[)", #selector(inspectBack)),
            (inspectionForward, "chevron.right", "Forward (⌘])", #selector(inspectForward)),
            (inspectionFind, "magnifyingglass", "Find in Current Page (⌘F)", #selector(findInInspection))
        ] {
            control.image = NSImage(systemSymbolName: symbol, accessibilityDescription: label)
            control.imagePosition = .imageOnly; control.bezelStyle = .rounded
            control.target = self; control.action = action; control.title = ""
            control.toolTip = label; control.setAccessibilityLabel(label)
            control.setContentHuggingPriority(.required, for: .horizontal)
        }
        inspectionBack.isEnabled = false; inspectionForward.isEnabled = false
        toolbarHistory.setEnabled(false, forSegment: 0)
        toolbarHistory.setEnabled(false, forSegment: 1)
        updateJumpBar()
        let inspectorBar = stack([inspectionBack, inspectionForward, inspectorTitle, inspectionFind, refreshInspect], spacing: 4)
        inspector.addArrangedSubview(inspectorBar)
        inspectorBar.widthAnchor.constraint(equalTo: inspector.widthAnchor).isActive = true
        breadcrumb.pathStyle = .standard; breadcrumb.isEditable = false; breadcrumb.target = self; breadcrumb.action = #selector(openBreadcrumb)
        breadcrumb.setContentCompressionResistancePriority(.init(100), for: .horizontal)
        breadcrumb.setContentHuggingPriority(.init(100), for: .horizontal)
        breadcrumb.setAccessibilityLabel("Inspected file path"); breadcrumb.isHidden = true
        inspector.addArrangedSubview(breadcrumb); breadcrumb.widthAnchor.constraint(equalTo: inspector.widthAnchor).isActive = true
        diffScope.target = self; diffScope.action = #selector(changeDiffScope); diffScope.isHidden = true; diffScope.selectedSegment = 0
        inspector.addArrangedSubview(diffScope)
        recordedOutput.target = self; recordedOutput.action = #selector(openRecordedOutput); recordedOutput.bezelStyle = .rounded; recordedOutput.isHidden = true
        relatedEvidence.target = self; relatedEvidence.action = #selector(openRelatedEvidence); relatedEvidence.isHidden = true
        relatedEvidence.setAccessibilityLabel("Related evidence for this instruction")
        relatedEvidence.toolTip = "Open activity, the previous instruction, or a recorded checkpoint"
        checkCode.target = self; checkCode.action = #selector(showCodeCheck); checkCode.bezelStyle = .rounded; checkCode.isHidden = true
        checkCode.toolTip = "Check saved JavaScript or TypeScript syntax, local imports and dependency cycles"
        prepareCodeReview.target = self; prepareCodeReview.action = #selector(draftCodeReview); prepareCodeReview.bezelStyle = .rounded; prepareCodeReview.isHidden = true
        prepareCodeReview.toolTip = "Add the findings to a review draft without sending it"
        let evidenceBar = stack([recordedOutput, checkCode, relatedEvidence, prepareCodeReview], spacing: 6)
        inspector.addArrangedSubview(evidenceBar)
        outline.target = self; outline.action = #selector(jumpToDeclaration); outline.isHidden = true
        outline.setAccessibilityLabel("Declarations in the displayed code page")
        inspector.addArrangedSubview(outline); outline.widthAnchor.constraint(equalTo: inspector.widthAnchor).isActive = true
        inspection.isEditable = false; inspection.isSelectable = true; inspection.isRichText = false
        inspection.delegate = self
        inspection.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        inspection.textContainerInset = NSSize(width: 12, height: 12)
        inspection.isAutomaticLinkDetectionEnabled = false
        inspection.usesFindBar = true; inspection.isIncrementalSearchingEnabled = true
        inspection.setAccessibilityLabel("Read-only file, diff or recorded evidence")
        inspectionScroll.documentView = inspection; inspectionScroll.hasVerticalScroller = true
        inspectionScroll.contentView.postsBoundsChangedNotifications = true
        viewportObserver = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification, object: inspectionScroll.contentView, queue: .main) { [weak self] _ in self?.scheduleViewportSave() }
        inspectionScroll.autohidesScrollers = true
        inspection.isVerticallyResizable = true; inspection.autoresizingMask = [.width]
        inspector.addArrangedSubview(inspectionScroll)
        documentView.isHidden = true
        documentView.onChange = { [weak self] in self?.scheduleViewportSave() }
        inspector.addArrangedSubview(documentView)
        documentView.widthAnchor.constraint(equalTo: inspector.widthAnchor).isActive = true
        documentView.setContentHuggingPriority(.init(1), for: .vertical); documentView.setContentCompressionResistancePriority(.init(1), for: .vertical)
        moreButton.target = self; moreButton.action = #selector(loadNextPage); moreButton.bezelStyle = .rounded; moreButton.isHidden = true
        previousButton.target = self; previousButton.action = #selector(loadPreviousPage); previousButton.bezelStyle = .rounded; previousButton.isEnabled = false
        pageLabel.font = .systemFont(ofSize: 10); pageLabel.textColor = .secondaryLabelColor; pageLabel.lineBreakMode = .byTruncatingTail
        revealButton.target = self; revealButton.action = #selector(revealFile); revealButton.bezelStyle = .rounded; revealButton.isHidden = true
        revealButton.title = "Reveal"; revealButton.toolTip = "Reveal in Finder"; revealButton.setAccessibilityLabel("Reveal in Finder")
        for control in [previousButton, moreButton, revealButton] { control.setContentHuggingPriority(.required, for: .horizontal) }
        pageLabel.setContentHuggingPriority(.init(100), for: .horizontal)
        pageLabel.setContentCompressionResistancePriority(.init(100), for: .horizontal)
        let paging = stack([previousButton, moreButton, revealButton, pageLabel], spacing: 6)
        inspector.addArrangedSubview(paging); paging.widthAnchor.constraint(equalTo: inspector.widthAnchor).isActive = true
        inspector.isHidden = true
        inspectionScroll.widthAnchor.constraint(equalTo: inspector.widthAnchor).isActive = true
        transcriptScroll.widthAnchor.constraint(greaterThanOrEqualToConstant: 270).isActive = true

        turnScrubber.onSelectTurn = { [weak self] rowIndex in
            guard let self, self.rows.indices.contains(rowIndex) else { return }
            self.transcript.scrollRowToVisible(rowIndex)
            self.jumpButton.isHidden = self.runs.isEmpty || self.transcriptScroll.documentVisibleRect.maxY >= self.transcript.bounds.height - 35
        }
        let transcriptArea = stack([turnScrubber, transcriptScroll], vertical: false, spacing: 6)
        transcriptScroll.setContentHuggingPriority(.init(1), for: .horizontal)

        surfaces.selectedSegment = 0; surfaces.target = self; surfaces.action = #selector(switchSurface); surfaces.isHidden = true
        let transcriptNavigation = stack([NSView(), jumpButton], spacing: 8)
        transcriptNavigation.heightAnchor.constraint(equalToConstant: 28).isActive = true
        let work = stack([transcriptArea, transcriptNavigation], vertical: true, spacing: 0)
        transcriptArea.widthAnchor.constraint(equalTo: work.widthAnchor).isActive = true
        transcriptNavigation.widthAnchor.constraint(equalTo: work.widthAnchor).isActive = true

        approval.orientation = .vertical; approval.alignment = .leading; approval.spacing = 6; approval.isHidden = true
        let approvalTitle = NSTextField(labelWithString: "Approval required")
        approvalTitle.font = .systemFont(ofSize: 13, weight: .semibold)
        approval.addArrangedSubview(approvalTitle)
        approvalText.isEditable = false; approvalText.isSelectable = true; approvalText.isRichText = false
        approvalText.font = .systemFont(ofSize: 12)
        approvalText.setAccessibilityLabel("Operation awaiting your review")
        let approvalScroll = NSScrollView(); approvalScroll.documentView = approvalText; approvalScroll.hasVerticalScroller = true
        approvalText.isVerticallyResizable = true; approvalText.autoresizingMask = [.width]
        approval.addArrangedSubview(approvalScroll)
        approvalScroll.heightAnchor.constraint(equalToConstant: 42).isActive = true
        approvalScroll.widthAnchor.constraint(equalTo: approval.widthAnchor).isActive = true
        reviewApprovalButton.target = self; reviewApprovalButton.action = #selector(showApprovalReview); reviewApprovalButton.bezelStyle = .rounded
        reviewApprovalButton.toolTip = "Review the full operation before deciding (⌘5)"
        denyButton.target = self; denyButton.action = #selector(deny); denyButton.bezelStyle = .rounded
        denyButton.toolTip = "Deny this operation"
        allowButton.target = self; allowButton.action = #selector(allowOnce); allowButton.bezelStyle = .rounded
        allowButton.bezelColor = .controlAccentColor
        allowButton.isEnabled = false
        allowButton.toolTip = "Allow this operation once without opening full review"
        denyButton.isEnabled = false
        approval.addArrangedSubview(stack([denyButton, reviewApprovalButton, allowButton]))

        composer.font = .systemFont(ofSize: 14); composer.isRichText = false; composer.delegate = self
        composer.isAutomaticQuoteSubstitutionEnabled = false; composer.isAutomaticDashSubstitutionEnabled = false
        composer.textContainerInset = NSSize(width: 0, height: 3); composer.drawsBackground = false
        composer.setAccessibilityLabel("Instruction to LUMI")
        composer.submit = { [weak self] in self?.submit() }
        composer.onFileDropped = { [weak self] url in
            guard let self else { return }
            let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.int64Value ?? 0
            if size > 20 * 1024 * 1024 {
                self.showNotice("Attachments are limited to 20MB. Choose a smaller file.")
                return
            }
            self.attachmentBar.add(url: url)
            self.saveDraft()
        }
        attachmentBar.onChange = { [weak self] in self?.saveDraft() }
        let composerScroll = NSScrollView(); composerScroll.documentView = composer; composerScroll.hasVerticalScroller = true
        composerScroll.borderType = .noBorder
        composerScroll.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
        composerScroll.heightAnchor.constraint(lessThanOrEqualToConstant: 160).isActive = true
        let composerBody = stack([attachmentBar, composerScroll], vertical: true, spacing: 6)
        attachmentBar.widthAnchor.constraint(equalTo: composerBody.widthAnchor).isActive = true
        composerScroll.widthAnchor.constraint(equalTo: composerBody.widthAnchor).isActive = true

        let composerBottomBar = NSStackView()
        composerBottomBar.orientation = .horizontal; composerBottomBar.alignment = .centerY; composerBottomBar.spacing = 8
        composerBottomBar.translatesAutoresizingMaskIntoConstraints = false
        status.setContentHuggingPriority(.init(1), for: .horizontal)
        status.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        composerBottomBar.addArrangedSubview(status)
        queueButton.target = self; queueButton.action = #selector(showQueue); queueButton.bezelStyle = .rounded
        queueButton.toolTip = "Review, edit, or remove queued instructions (⌘4)"
        queueButton.setContentCompressionResistancePriority(.required, for: .horizontal)
        composerBottomBar.addArrangedSubview(queueButton)
        queuePopover.behavior = .transient; queuePopover.contentViewController = queueContent
        queueContent.onRemove = { [weak self] in self?.removeQueued($0) }
        sendButton.target = self; sendButton.action = #selector(submit); sendButton.bezelStyle = .rounded
        sendButton.bezelColor = .controlAccentColor
        sendButton.keyEquivalent = "\r"
        sendButton.setAccessibilityLabel("Send instruction to LUMI")
        stopSendButton.target = self; stopSendButton.action = #selector(stopAndSend); stopSendButton.bezelStyle = .rounded
        stopSendButton.isHidden = true
        composerBottomBar.addArrangedSubview(stopSendButton)
        composerBottomBar.addArrangedSubview(stopButton)
        composerBottomBar.addArrangedSubview(sendButton)
        for control in [queueButton, stopSendButton, stopButton, sendButton] { control.setContentHuggingPriority(.required, for: .horizontal) }

        let composerSurface = NSStackView(views: [composerBody, composerBottomBar])
        composerSurface.orientation = .vertical; composerSurface.alignment = .leading; composerSurface.spacing = 8
        composerSurface.edgeInsets = NSEdgeInsets(top: 10, left: 12, bottom: 10, right: 12)
        composerSurface.wantsLayer = true; composerSurface.layer?.cornerRadius = 8
        composerSurface.layer?.borderWidth = 1; composerSurface.layer?.borderColor = NSColor.separatorColor.cgColor
        composerSurface.layer?.backgroundColor = NSColor.controlBackgroundColor.cgColor
        composerBody.widthAnchor.constraint(equalTo: composerSurface.widthAnchor, constant: -24).isActive = true
        composerBottomBar.widthAnchor.constraint(equalTo: composerSurface.widthAnchor, constant: -24).isActive = true

        let dismissNotice = button("", #selector(dismissNotice), symbol: "xmark")
        dismissNotice.bezelStyle = .inline; dismissNotice.toolTip = "Dismiss notice"
        dismissNotice.widthAnchor.constraint(equalToConstant: 16).isActive = true
        dismissNotice.heightAnchor.constraint(equalToConstant: 16).isActive = true
        noticeBar.orientation = .horizontal; noticeBar.alignment = .centerY; noticeBar.spacing = 8
        noticeBar.edgeInsets = NSEdgeInsets(top: 6, left: 10, bottom: 6, right: 10)
        noticeBar.wantsLayer = true; noticeBar.layer?.cornerRadius = 6
        noticeBar.layer?.backgroundColor = NSColor.systemOrange.withAlphaComponent(0.12).cgColor
        notice.font = .systemFont(ofSize: 12); notice.textColor = .labelColor
        notice.setContentHuggingPriority(.init(1), for: .horizontal)
        reconnectButton.target = self; reconnectButton.action = #selector(reconnect); reconnectButton.bezelStyle = .rounded; reconnectButton.isHidden = true
        [notice, reconnectButton, dismissNotice].forEach(noticeBar.addArrangedSubview)
        noticeBar.isHidden = true

        modelButton.target = self; modelButton.action = #selector(showModels); modelButton.bezelStyle = .rounded
        modelButton.setAccessibilityLabel("Choose model and provider")
        modelButton.image = NSImage(systemSymbolName: "chevron.down", accessibilityDescription: nil); modelButton.imagePosition = .imageTrailing
        modelButton.setContentHuggingPriority(.required, for: .horizontal)
        modelButton.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        modelButton.cell?.lineBreakMode = .byTruncatingMiddle

        checkpointsButton.target = self; checkpointsButton.action = #selector(showCheckpoints); checkpointsButton.bezelStyle = .rounded
        checkpointsButton.image = NSImage(systemSymbolName: "clock.arrow.circlepath", accessibilityDescription: nil); checkpointsButton.imagePosition = .imageLeading
        checkpointsButton.toolTip = "Restore project files before or after a message"
        let titleLine = stack([chatTitle, checkpointsButton], spacing: 8)
        chatTitle.setContentHuggingPriority(.init(1), for: .horizontal); checkpointsButton.setContentCompressionResistancePriority(.required, for: .horizontal)
        let header = stack([titleLine, chatState], vertical: true, spacing: 5)
        let rule = JourneyStyle.rule()
        reasoningPicker.request = { [weak self] method, params, completion in
            guard let self else { completion(.failure(BridgeError.message("Workspace closed."))); return }
            self.client.request(method, params: params, completion: completion)
        }
        reasoningPicker.canApply = { [weak self] in
            guard let self else { return false }
            return self.canRequestRecovery && self.stopThenSend == nil && self.window?.attachedSheet == nil
        }
        reasoningPicker.onSelection = { [weak self] data in
            if let selection = WorkspaceModelSelection(data) { self?.updateModel(selection) }
            self?.modelPicker?.receive(data)
        }
        reasoningPicker.onPending = { [weak self] in self?.updateStatus() }
        reasoningPicker.onError = { [weak self] in self?.showNotice($0) }
        let modelBar = stack([modelButton, reasoningPicker, hint], spacing: 8)
        hint.toolTip = "Enter to send · Shift–Enter for a new line. Autonomous execution runs tools without approval prompts. Use Stop to interrupt."
        modelButton.widthAnchor.constraint(lessThanOrEqualTo: modelBar.widthAnchor, multiplier: 0.45).isActive = true
        modelButton.widthAnchor.constraint(lessThanOrEqualToConstant: 260).isActive = true
        modelButton.widthAnchor.constraint(greaterThanOrEqualToConstant: 80).isActive = true
        let center = stack([surfaces, header, rule, work, approval, noticeBar, composerSurface, modelBar], vertical: true, spacing: 10)
        center.setContentHuggingPriority(.init(1), for: .horizontal)
        work.setContentHuggingPriority(.init(1), for: .vertical)
        center.edgeInsets = NSEdgeInsets(top: 20, left: 22, bottom: 16, right: 22)
        for view in [header, rule, work, approval, noticeBar, composerSurface, modelBar] { view.widthAnchor.constraint(equalTo: center.widthAnchor, constant: -44).isActive = true }
        titleLine.widthAnchor.constraint(equalTo: header.widthAnchor).isActive = true
        chatState.widthAnchor.constraint(equalTo: header.widthAnchor).isActive = true
        work.heightAnchor.constraint(greaterThanOrEqualToConstant: 60).isActive = true
        buildContextPanel()
        contextWidth = contextPanel.widthAnchor.constraint(equalToConstant: 350); contextWidth?.isActive = true
        contextSeparator.widthAnchor.constraint(equalToConstant: 1).isActive = true
        let body = stack([leftSidebar, center, contextSeparator, contextPanel], spacing: 0)
        body.alignment = .top
        center.heightAnchor.constraint(equalTo: body.heightAnchor).isActive = true
        leftSidebar.heightAnchor.constraint(equalTo: body.heightAnchor).isActive = true
        conversations.heightAnchor.constraint(equalTo: leftNavigatorStack.heightAnchor, constant: -44).isActive = true
        navigation.heightAnchor.constraint(equalTo: leftNavigatorStack.heightAnchor, constant: -44).isActive = true
        contextPanel.heightAnchor.constraint(equalTo: body.heightAnchor).isActive = true
        contextSeparator.heightAnchor.constraint(equalTo: body.heightAnchor).isActive = true
        let root = stack([body], vertical: true, spacing: 0)
        content.addSubview(root)
        NSLayoutConstraint.activate([root.leadingAnchor.constraint(equalTo: content.leadingAnchor), root.trailingAnchor.constraint(equalTo: content.trailingAnchor), root.topAnchor.constraint(equalTo: content.topAnchor), root.bottomAnchor.constraint(equalTo: content.bottomAnchor), body.widthAnchor.constraint(equalTo: root.widthAnchor)])
        rows = [welcomeRow]
        transcript.reloadData()
        adapt(); updateWalkthrough()
    }

    var welcomeRow: TranscriptRow {
        TranscriptRow(id: "welcome", title: "What would you like to work on?", text: "Describe what you want to build, fix, or understand. LUMI will work with you here.\n\nFollow its activity in Walkthrough. Browse project files and review changes in Task context.", activity: false)
    }

    private func buildContextPanel() {
        contextPanel.orientation = .vertical; contextPanel.alignment = .leading; contextPanel.spacing = 10
        contextPanel.edgeInsets = NSEdgeInsets(top: 20, left: 14, bottom: 16, right: 14)
        let title = JourneyStyle.label("Task context", size: 16, weight: .semibold)
        openFileButton.target = self; openFileButton.action = #selector(showQuickOpen); openFileButton.bezelStyle = .rounded
        openFileButton.toolTip = "Open a workspace file by name or path (⌘P)"
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

    @objc private func changeContext() {
        showingInspector = true; surfaces.selectedSegment = 1
        if contextTabs.selectedSegment == 0 { walkthrough.followCurrentInstruction() }
        else if contextTabs.selectedSegment == 1 { navigation.select(.changes) }
        else if contextTabs.selectedSegment == 2 { navigation.select(.files) }
        adapt()
    }

    @objc private func toggleContextBrowser() { browsingCompact.toggle(); adapt() }

    @objc func showWalkthrough() {
        walkthrough.followCurrentInstruction(); contextTabs.selectedSegment = 0; showingInspector = true; surfaces.selectedSegment = 1; switchSurface()
    }

    @objc func toggleMaximizeContext() {
        guard canMaximizeContext else { return }
        isContextMaximized.toggle()
        adapt()
    }

    @objc func switchSurface() {
        if surfaces.selectedSegment == 1, (window?.frame.width ?? 1280) < 1000 {
            compactSidebarRevealed = false; showingInspector = true
        }
        adapt()
    }

    func adapt() {
        let narrow = (window?.frame.width ?? 1280) < 1000
        let short = (window?.frame.height ?? 800) < 640
        navigation.previewsSelection = !short
        leftSidebar.isHidden = narrow ? !compactSidebarRevealed : !sidebarPreferred
        conversations.isHidden = leftSidebar.isHidden || navigatorTabs.selectedSegment != 0
        navigation.isHidden = leftSidebar.isHidden || navigatorTabs.selectedSegment == 0
        surfaces.isHidden = !narrow
        contextPanel.isHidden = !showingInspector || (narrow && (surfaces.selectedSegment == 0 || compactSidebarRevealed))
        contextSeparator.isHidden = contextPanel.isHidden
        let isPreviewActive = contextTabs.selectedSegment == 3 && projectRun.isInAppPreviewVisible
        let defaultContextWidth = narrow ? 300.0 : min(400.0, max(330.0, (window?.frame.width ?? 1280.0) * 0.27))
        let targetWidth: CGFloat
        if isContextMaximized {
            targetWidth = max(680.0, (window?.frame.width ?? 1280.0) - (narrow ? 20.0 : 280.0))
        } else if isPreviewActive {
            targetWidth = max(680.0, min(800.0, (window?.frame.width ?? 1280.0) * 0.52))
        } else {
            targetWidth = defaultContextWidth
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

    func restoreTranscriptAfterResize() {
        guard let anchor = resizeAnchor else { return }
        window?.contentView?.layoutSubtreeIfNeeded()
        transcript.noteHeightOfRows(withIndexesChanged: IndexSet(integersIn: 0..<rows.count))
        transcript.layoutSubtreeIfNeeded()
        if anchor.following { jumpLatest() }
        else {
            transcriptScroll.contentView.scroll(to: anchor.origin)
            transcriptScroll.reflectScrolledClipView(transcriptScroll.contentView)
            jumpButton.isHidden = runs.isEmpty || transcriptScroll.documentVisibleRect.maxY >= transcript.bounds.height - 35
        }
        resizeAnchor = nil
    }

    func windowDidResize(_ notification: Notification) { adapt(); modelPicker?.syncPresentation(recenter: true) }
    func windowDidBecomeKey(_ notification: Notification) { if needsRebuild { rebuildRows() }; if client.isReady && !closing { client.requestSnapshot(); checkViewportBranch() } }
    func windowDidChangeOcclusionState(_ notification: Notification) { if needsRebuild && window?.occlusionState.contains(.visible) == true { rebuildRows() }; updateElapsedTimer(); updateWalkthroughVisibility() }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        guard prepareToClose() else { return false }
        if stopped || client.process == nil { return true }
        if closing { return false }
        if hasWork {
            let alert = NSAlert(); alert.messageText = "Stop work and close this workspace?"
            alert.informativeText = "Keep Running leaves the window open. Stop and Close waits for LUMI to finish cancellation. Existing edits are retained."
            alert.addButton(withTitle: "Keep Running"); alert.addButton(withTitle: "Stop and Close")
            if alert.runModal() != .alertSecondButtonReturn { return false }
        }
        stopAndClose(); return false
    }

    func stopAndClose() {
        guard !closing else { return }
        guard prepareToClose() else { return }
        if stopped || client.process == nil { window?.close(); return }
        closing = true; composer.isEditable = false; stopThenSend = nil; status.stringValue = "Stopping and flushing workspace…"; stop(); client.shutdown()
    }

    func windowWillClose(_ notification: Notification) {
        dispose()
        saveViewport()
        saveDraft()
        client.closeInput()
        onClosed?()
    }

    func numberOfRows(in tableView: NSTableView) -> Int { rows.count }

    func tableView(_ tableView: NSTableView, heightOfRow row: Int) -> CGFloat {
        guard rows.indices.contains(row) else { return 40 }
        return TranscriptMessageLayout(rows[row], availableWidth: tableView.frameOfCell(atColumn: 0, row: row).width).height
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let value = rows[row]
        return TranscriptMessageCell(value, expanded: expandedActions.contains(value.id), toggle: { [weak self] in
            guard let self else { return }
            if self.expandedActions.contains(value.id) { self.expandedActions.remove(value.id) } else { self.expandedActions.insert(value.id) }
            self.rebuildRows()
        }, openCheckpoint: { [weak self] in
            if let checkpoint = value.checkpoint { self?.openCheckpoints(selected: checkpoint) }
        }, openAction: { [weak self] in
            guard let self else { return }
            self.showWalkthrough()
            if let runID = value.runID {
                if runID == self.active { self.walkthrough.followCurrentInstruction() }
                else { self.walkthrough.selectRecordedRun(runID) }
            }
            if let activityID = value.activityID {
                self.walkthrough.selectActivity(activityID, expand: true)
            }
        })
    }
}
