// [LAYER: UI]
import AppKit

/// Dedicated full-featured Git & Source Control window with staged/working-tree file lists,
/// commit message generation, branch management, push/pull/sync, live diff viewer, and commit graph.
final class SourceControlWindow: NSWindowController, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate, NSTextFieldDelegate {
    private struct ChangeItem {
        let isSectionHeader: Bool
        let title: String
        let path: String
        let statusBadge: String
        let staged: Bool
        let unstaged: Bool
        let untracked: Bool
        let sectionCount: Int
    }

    private let folder: URL
    private let request: (String, [String: Any], @escaping (Result<[String: Any], Error>) -> Void) -> Void
    private var items = [ChangeItem]()
    private var isGitRepo = false
    private var currentBranch = "unversioned"
    private var hasRemote = false
    private var remoteWebURL = ""
    private var remoteDisplayName = ""
    private var aheadCount = 0
    private var behindCount = 0
    private var selectedPath: String?
    private var selectedStaged = false

    private let repoLabel = NSTextField(labelWithString: "")
    private let branchPopup = NSPopUpButton(frame: .zero, pullsDown: false)
    private let syncStatusLabel = NSTextField(labelWithString: "")
    private let noticeLabel = NSTextField(labelWithString: "")
    private let initBanner = NSStackView()
    private let remoteBanner = SourceControlRemoteBar()
    private let commitField = NSTextField(string: "")
    private let table = NSTableView()
    private let diffMode = NSSegmentedControl(labels: ["Working Tree", "Staged"], trackingMode: .selectOne, target: nil, action: nil)
    private let diffTitleLabel = NSTextField(labelWithString: "Select a file or commit to inspect its diff")
    private let diffView = NSTextView()
    private let graphView = SourceControlGraphView()
    private let splitView = NSSplitView()
    private let rightSplit = NSSplitView()
    private var initBannerHeight: NSLayoutConstraint?
    private var remoteBannerHeight: NSLayoutConstraint?

    private var generateBtn: JourneyButton!
    private var commitBtn: JourneyButton!
    private var commitPushBtn: JourneyButton!
    private var amendBtn: JourneyButton!
    private var fetchBtn: JourneyButton!
    private var pullBtn: JourneyButton!
    private var pushBtn: JourneyButton!
    private var syncBtn: JourneyButton!
    private var stashBtn: JourneyButton!
    private var popStashBtn: JourneyButton!
    private var newBranchBtn: JourneyButton!
    private var openGitHubBtn: JourneyButton!
    private var stageAllBtn: JourneyButton!
    private var unstageAllBtn: JourneyButton!
    private var refreshBtn: JourneyButton!

    var onClose: (() -> Void)?

    init(folder: URL, request: @escaping (String, [String: Any], @escaping (Result<[String: Any], Error>) -> Void) -> Void) {
        self.folder = folder
        self.request = request
        let win = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 980, height: 660),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        win.title = "Source Control — \(folder.lastPathComponent)"
        win.minSize = NSSize(width: 760, height: 500)
        win.isReleasedWhenClosed = false
        super.init(window: win)
        win.delegate = self
        buildUI(in: win)
        refreshAll()
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    private func buildUI(in win: NSWindow) {
        guard let root = win.contentView else { return }
        repoLabel.font = .systemFont(ofSize: 13, weight: .semibold)
        repoLabel.stringValue = folder.lastPathComponent
        branchPopup.controlSize = .small
        branchPopup.target = self; branchPopup.action = #selector(switchBranch)
        newBranchBtn = JourneyButton("New Branch…", symbol: "plus") { [weak self] in self?.promptNewBranch() }
        newBranchBtn.controlSize = .small; newBranchBtn.bezelStyle = .rounded
        openGitHubBtn = JourneyButton("GitHub", symbol: "arrow.up.right.square") { [weak self] in
            guard let self, let url = URL(string: self.remoteWebURL), !self.remoteWebURL.isEmpty else { return }
            NSWorkspace.shared.open(url)
        }
        openGitHubBtn.controlSize = .small; openGitHubBtn.bezelStyle = .rounded; openGitHubBtn.isHidden = true
        syncStatusLabel.font = .systemFont(ofSize: 11); syncStatusLabel.textColor = .secondaryLabelColor
        syncStatusLabel.setContentHuggingPriority(.init(1), for: .horizontal)

        fetchBtn = JourneyButton("Fetch", symbol: "arrow.down.to.line") { [weak self] in self?.runGit("git.fetch", params: [:], notice: "Fetched remote.") }
        pullBtn = JourneyButton("Pull", symbol: "arrow.down.circle") { [weak self] in self?.runGit("git.pull", params: [:], notice: "Pulled changes.") }
        pushBtn = JourneyButton("Push", symbol: "arrow.up.circle") { [weak self] in self?.runGit("git.push", params: [:], notice: "Pushed to remote.") }
        syncBtn = JourneyButton("Sync", symbol: "arrow.triangle.2.circlepath") { [weak self] in self?.runGit("git.sync", params: [:], notice: "Synced with remote.") }
        stashBtn = JourneyButton("Stash", symbol: "tray.and.arrow.down") { [weak self] in self?.runGit("git.stash", params: [:], notice: "Stashed changes.") }
        popStashBtn = JourneyButton("Pop Stash", symbol: "tray.and.arrow.up") { [weak self] in self?.runGit("git.stashPop", params: [:], notice: "Applied latest stash.") }
        refreshBtn = JourneyButton("", symbol: "arrow.clockwise") { [weak self] in self?.refreshAll() }
        for b in [fetchBtn!, pullBtn!, pushBtn!, syncBtn!, stashBtn!, popStashBtn!, refreshBtn!] {
            b.controlSize = .small; b.bezelStyle = .rounded
        }
        let toolbar = NSStackView(views: [repoLabel, branchPopup, newBranchBtn, openGitHubBtn, syncStatusLabel, fetchBtn, pullBtn, pushBtn, syncBtn, stashBtn, popStashBtn, refreshBtn])
        toolbar.orientation = .horizontal; toolbar.alignment = .centerY; toolbar.spacing = 6
        toolbar.edgeInsets = NSEdgeInsets(top: 10, left: 14, bottom: 8, right: 14)
        toolbar.translatesAutoresizingMaskIntoConstraints = false

        let initLabel = NSTextField(labelWithString: "This workspace is not a Git repository yet.")
        initLabel.font = .systemFont(ofSize: 12); initLabel.textColor = .secondaryLabelColor
        initLabel.setContentHuggingPriority(.init(1), for: .horizontal)
        let initBtn = JourneyButton("Initialize Git Repository", symbol: "arrow.triangle.branch") { [weak self] in
            self?.runGit("git.init", params: [:], notice: "Initialized Git repository.")
        }
        initBtn.bezelStyle = .rounded; initBtn.bezelColor = .controlAccentColor
        initBanner.orientation = .horizontal; initBanner.alignment = .centerY; initBanner.spacing = 8
        initBanner.edgeInsets = NSEdgeInsets(top: 8, left: 14, bottom: 8, right: 14)
        initBanner.wantsLayer = true; initBanner.layer?.backgroundColor = NSColor.controlAccentColor.withAlphaComponent(0.08).cgColor
        [initLabel, initBtn].forEach(initBanner.addArrangedSubview)
        initBanner.isHidden = true
        initBanner.translatesAutoresizingMaskIntoConstraints = false

        // One-click on-the-fly GitHub repository creation bar
        remoteBanner.onCreateAndPublish = { [weak self] repoName, visibility in
            guard let self else { return }
            self.noticeLabel.stringValue = "Creating new GitHub repository and uploading your project…"
            var params: [String: Any] = ["visibility": visibility]
            if !repoName.isEmpty { params["name"] = repoName }
            self.request("git.publishGitHub", params) { [weak self] result in
                guard let self else { return }
                self.remoteBanner.setPublishing(false)
                switch result {
                case .success(let data):
                    if data["ok"] as? Bool == false {
                        self.noticeLabel.stringValue = data["error"] as? String ?? "Could not create GitHub repository."
                        return
                    }
                    self.noticeLabel.stringValue = data["summary"] as? String ?? "Created and published repository on GitHub."
                    self.refreshAll()
                case .failure(let err):
                    self.noticeLabel.stringValue = err.localizedDescription
                }
            }
        }
        remoteBanner.onConnectRepo = { [weak self] target in
            self?.noticeLabel.stringValue = "Connecting your project to GitHub…"
            self?.runGit("git.connectRepo", params: ["url": target], notice: "Connected to GitHub.")
        }
        remoteBanner.onSignIn = { [weak self] in
            self?.noticeLabel.stringValue = "Opening GitHub sign-in…"
            self?.request("git.githubSignIn", [:]) { [weak self] result in
                guard let self else { return }
                switch result {
                case .success(let data):
                    if let openPath = data["openPath"] as? String, !openPath.isEmpty {
                        NSWorkspace.shared.open(URL(fileURLWithPath: openPath))
                    } else if let openUrl = data["openUrl"] as? String, let url = URL(string: openUrl) {
                        NSWorkspace.shared.open(url)
                    } else if let fallback = URL(string: "https://github.com/login") {
                        NSWorkspace.shared.open(fallback)
                    }
                    self.noticeLabel.stringValue = data["summary"] as? String ?? "Opened GitHub sign-in in your browser."
                case .failure:
                    if let fallback = URL(string: "https://github.com/login") {
                        NSWorkspace.shared.open(fallback)
                    }
                    self.noticeLabel.stringValue = "Opened GitHub in your browser."
                }
            }
        }
        remoteBanner.onCheckStatus = { [weak self] in
            self?.noticeLabel.stringValue = "Checking GitHub sign-in…"
            self?.loadGitHubStatus()
        }
        remoteBanner.isHidden = true
        remoteBanner.translatesAutoresizingMaskIntoConstraints = false

        // Left Pane: Commit box + Changes list
        let leftPane = NSView()
        commitField.placeholderString = "Commit message (⌘↩ to commit)"
        commitField.font = .systemFont(ofSize: 12)
        commitField.delegate = self
        generateBtn = JourneyButton("Generate", symbol: "sparkles") { [weak self] in self?.generateCommitMessage() }
        generateBtn.controlSize = .small; generateBtn.bezelStyle = .rounded; generateBtn.bezelColor = .controlAccentColor
        let msgRow = NSStackView(views: [commitField, generateBtn])
        msgRow.orientation = .horizontal; msgRow.alignment = .centerY; msgRow.spacing = 6
        commitField.setContentHuggingPriority(.init(1), for: .horizontal)

        commitBtn = JourneyButton("Commit", symbol: "checkmark") { [weak self] in self?.commit(push: false, amend: false) }
        commitBtn.controlSize = .small; commitBtn.bezelStyle = .rounded; commitBtn.bezelColor = .controlAccentColor
        commitPushBtn = JourneyButton("Commit & Push", symbol: "arrow.up.circle.fill") { [weak self] in self?.commit(push: true, amend: false) }
        commitPushBtn.controlSize = .small; commitPushBtn.bezelStyle = .rounded
        amendBtn = JourneyButton("Amend", symbol: "pencil") { [weak self] in self?.commit(push: false, amend: true) }
        amendBtn.controlSize = .small; amendBtn.bezelStyle = .rounded
        stageAllBtn = JourneyButton("Stage All", symbol: "plus") { [weak self] in self?.runGit("git.stage", params: ["all": true]) }
        stageAllBtn.controlSize = .small; stageAllBtn.bezelStyle = .rounded
        unstageAllBtn = JourneyButton("Unstage All", symbol: "minus") { [weak self] in self?.runGit("git.unstage", params: ["all": true]) }
        unstageAllBtn.controlSize = .small; unstageAllBtn.bezelStyle = .rounded
        let btnRow1 = NSStackView(views: [commitBtn, commitPushBtn, amendBtn])
        btnRow1.orientation = .horizontal; btnRow1.spacing = 6
        let btnRow2 = NSStackView(views: [stageAllBtn, unstageAllBtn])
        btnRow2.orientation = .horizontal; btnRow2.spacing = 6

        table.addTableColumn(NSTableColumn(identifier: NSUserInterfaceItemIdentifier("change")))
        table.headerView = nil; table.dataSource = self; table.delegate = self
        table.style = .sourceList; table.backgroundColor = .clear; table.rowHeight = 26
        table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        let tableScroll = NSScrollView(); tableScroll.documentView = table; tableScroll.hasVerticalScroller = true; tableScroll.drawsBackground = false
        tableScroll.setContentHuggingPriority(.init(1), for: .vertical)
        tableScroll.setContentCompressionResistancePriority(.init(1), for: .vertical)

        let leftStack = NSStackView(views: [msgRow, btnRow1, btnRow2, tableScroll])
        leftStack.orientation = .vertical; leftStack.alignment = .leading; leftStack.spacing = 8
        leftStack.edgeInsets = NSEdgeInsets(top: 10, left: 12, bottom: 10, right: 12)
        leftStack.translatesAutoresizingMaskIntoConstraints = false
        leftPane.addSubview(leftStack)
        NSLayoutConstraint.activate([
            leftStack.topAnchor.constraint(equalTo: leftPane.topAnchor),
            leftStack.bottomAnchor.constraint(equalTo: leftPane.bottomAnchor),
            leftStack.leadingAnchor.constraint(equalTo: leftPane.leadingAnchor),
            leftStack.trailingAnchor.constraint(equalTo: leftPane.trailingAnchor),
            msgRow.widthAnchor.constraint(equalTo: leftStack.widthAnchor, constant: -24),
            tableScroll.widthAnchor.constraint(equalTo: leftStack.widthAnchor, constant: -24)
        ])

        // Right Pane: Diff viewer (top) + Commit Graph (bottom)
        let diffContainer = NSView()
        diffTitleLabel.font = .systemFont(ofSize: 12, weight: .semibold)
        diffTitleLabel.lineBreakMode = .byTruncatingMiddle
        diffTitleLabel.setContentHuggingPriority(.init(1), for: .horizontal)
        diffMode.selectedSegment = 0; diffMode.controlSize = .small
        diffMode.target = self; diffMode.action = #selector(diffScopeChanged)
        let diffHeader = NSStackView(views: [diffTitleLabel, diffMode])
        diffHeader.orientation = .horizontal; diffHeader.alignment = .centerY; diffHeader.spacing = 8
        diffHeader.translatesAutoresizingMaskIntoConstraints = false

        diffView.isEditable = false; diffView.isSelectable = true
        diffView.isVerticallyResizable = true; diffView.isHorizontallyResizable = false
        diffView.autoresizingMask = [.width]
        diffView.textContainer?.widthTracksTextView = true
        diffView.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        diffView.textContainerInset = NSSize(width: 8, height: 8)
        let diffScroll = NSScrollView(); diffScroll.documentView = diffView; diffScroll.hasVerticalScroller = true
        diffScroll.borderType = .noBorder; diffScroll.translatesAutoresizingMaskIntoConstraints = false
        diffScroll.setContentHuggingPriority(.init(1), for: .vertical)
        diffScroll.setContentCompressionResistancePriority(.init(1), for: .vertical)
        diffContainer.addSubview(diffHeader); diffContainer.addSubview(diffScroll)
        NSLayoutConstraint.activate([
            diffHeader.topAnchor.constraint(equalTo: diffContainer.topAnchor, constant: 8),
            diffHeader.leadingAnchor.constraint(equalTo: diffContainer.leadingAnchor, constant: 12),
            diffHeader.trailingAnchor.constraint(equalTo: diffContainer.trailingAnchor, constant: -12),
            diffScroll.topAnchor.constraint(equalTo: diffHeader.bottomAnchor, constant: 6),
            diffScroll.leadingAnchor.constraint(equalTo: diffContainer.leadingAnchor),
            diffScroll.trailingAnchor.constraint(equalTo: diffContainer.trailingAnchor),
            diffScroll.bottomAnchor.constraint(equalTo: diffContainer.bottomAnchor)
        ])

        graphView.onSelectCommit = { [weak self] hash in self?.loadCommitDiff(hash) }
        graphView.onFetch = { [weak self] in self?.runGit("git.fetch", params: [:], notice: "Fetched remote.") }
        graphView.onPull = { [weak self] in self?.runGit("git.pull", params: [:], notice: "Pulled changes.") }
        graphView.onPush = { [weak self] in self?.runGit("git.push", params: [:], notice: "Pushed to remote.") }
        graphView.onRefresh = { [weak self] in self?.refreshAll() }

        rightSplit.isVertical = false; rightSplit.dividerStyle = .thin
        rightSplit.addArrangedSubview(diffContainer)
        rightSplit.addArrangedSubview(graphView)

        splitView.isVertical = true; splitView.dividerStyle = .thin
        splitView.addArrangedSubview(leftPane)
        splitView.addArrangedSubview(rightSplit)
        splitView.translatesAutoresizingMaskIntoConstraints = false

        noticeLabel.font = .systemFont(ofSize: 11); noticeLabel.textColor = .secondaryLabelColor
        let noticeBar = NSStackView(views: [noticeLabel])
        noticeBar.orientation = .horizontal; noticeBar.alignment = .centerY
        noticeBar.edgeInsets = NSEdgeInsets(top: 5, left: 12, bottom: 6, right: 12)
        noticeBar.translatesAutoresizingMaskIntoConstraints = false

        let topRule = NSBox(); topRule.boxType = .separator; topRule.translatesAutoresizingMaskIntoConstraints = false
        let botRule = NSBox(); botRule.boxType = .separator; botRule.translatesAutoresizingMaskIntoConstraints = false
        [toolbar, initBanner, remoteBanner, topRule, splitView, botRule, noticeBar].forEach(root.addSubview)

        let bannerHeight = initBanner.heightAnchor.constraint(equalToConstant: 0)
        initBannerHeight = bannerHeight
        let remHeight = remoteBanner.heightAnchor.constraint(equalToConstant: 0)
        remoteBannerHeight = remHeight
        NSLayoutConstraint.activate([
            toolbar.topAnchor.constraint(equalTo: root.topAnchor),
            toolbar.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            toolbar.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            toolbar.heightAnchor.constraint(equalToConstant: 42),
            initBanner.topAnchor.constraint(equalTo: toolbar.bottomAnchor),
            initBanner.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            initBanner.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            bannerHeight,
            remoteBanner.topAnchor.constraint(equalTo: initBanner.bottomAnchor),
            remoteBanner.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            remoteBanner.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            remHeight,
            topRule.topAnchor.constraint(equalTo: remoteBanner.bottomAnchor),
            topRule.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            topRule.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            splitView.topAnchor.constraint(equalTo: topRule.bottomAnchor),
            splitView.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            splitView.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            splitView.bottomAnchor.constraint(equalTo: botRule.topAnchor),
            botRule.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            botRule.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            botRule.bottomAnchor.constraint(equalTo: noticeBar.topAnchor),
            noticeBar.leadingAnchor.constraint(equalTo: root.leadingAnchor),
            noticeBar.trailingAnchor.constraint(equalTo: root.trailingAnchor),
            noticeBar.bottomAnchor.constraint(equalTo: root.bottomAnchor),
            noticeBar.heightAnchor.constraint(equalToConstant: 26),
            leftPane.widthAnchor.constraint(greaterThanOrEqualToConstant: 290),
            diffContainer.heightAnchor.constraint(greaterThanOrEqualToConstant: 180),
            graphView.heightAnchor.constraint(greaterThanOrEqualToConstant: 150)
        ])
        DispatchQueue.main.async { [weak self] in
            self?.splitView.setPosition(340, ofDividerAt: 0)
            self?.rightSplit.setPosition(360, ofDividerAt: 0)
        }
    }

    func present() {
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        refreshAll()
    }

    func refreshAll() {
        request("changes.list", [:]) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let data):
                self.isGitRepo = data["isGitRepo"] as? Bool ?? false
                self.currentBranch = data["branch"] as? String ?? "unversioned"
                self.hasRemote = data["hasRemote"] as? Bool ?? false
                self.aheadCount = data["ahead"] as? Int ?? 0
                self.behindCount = data["behind"] as? Int ?? 0
                let raw = data["entries"] as? [[String: Any]] ?? []
                let staged = raw.filter { $0["staged"] as? Bool == true }
                let unstaged = raw.filter { $0["unstaged"] as? Bool == true || $0["untracked"] as? Bool == true || $0["staged"] as? Bool != true }
                var next = [ChangeItem]()
                if !staged.isEmpty {
                    next.append(ChangeItem(isSectionHeader: true, title: "STAGED CHANGES", path: "", statusBadge: "", staged: true, unstaged: false, untracked: false, sectionCount: staged.count))
                    for e in staged {
                        let st = e["status"] as? String ?? "modified"
                        let badge = st == "added" ? "A" : st == "deleted" ? "D" : st == "renamed" ? "R" : "M"
                        next.append(ChangeItem(isSectionHeader: false, title: e["name"] as? String ?? "File", path: e["path"] as? String ?? "", statusBadge: badge, staged: true, unstaged: false, untracked: false, sectionCount: 0))
                    }
                }
                if !unstaged.isEmpty {
                    next.append(ChangeItem(isSectionHeader: true, title: "CHANGES", path: "", statusBadge: "", staged: false, unstaged: true, untracked: false, sectionCount: unstaged.count))
                    for e in unstaged {
                        let st = e["status"] as? String ?? "modified"
                        let untracked = e["untracked"] as? Bool == true
                        let badge = untracked ? "U" : st == "deleted" ? "D" : st == "added" ? "A" : "M"
                        next.append(ChangeItem(isSectionHeader: false, title: e["name"] as? String ?? "File", path: e["path"] as? String ?? "", statusBadge: badge, staged: false, unstaged: true, untracked: untracked, sectionCount: 0))
                    }
                }
                self.items = next
                self.table.reloadData()
                if self.selectedPath == nil, let firstFileIndex = next.firstIndex(where: { !$0.isSectionHeader }) {
                    self.table.selectRowIndexes(IndexSet(integer: firstFileIndex), byExtendingSelection: false)
                    self.loadFileDiff(next[firstFileIndex].path, staged: next[firstFileIndex].staged)
                }
                self.updateControls(hasStaged: !staged.isEmpty, hasChanges: !raw.isEmpty)
                if self.isGitRepo {
                    self.loadBranches()
                    self.loadGraph()
                    self.loadGitHubStatus()
                } else {
                    self.graphView.update(commits: [], repoName: self.folder.lastPathComponent, hasRemote: false)
                }
            case .failure(let err):
                self.noticeLabel.stringValue = err.localizedDescription
            }
        }
    }

    private func updateControls(hasStaged: Bool, hasChanges: Bool) {
        initBanner.isHidden = isGitRepo
        initBannerHeight?.constant = isGitRepo ? 0 : 42
        let showRemoteBar = isGitRepo && !hasRemote
        remoteBanner.isHidden = !showRemoteBar
        remoteBannerHeight?.constant = showRemoteBar ? 38 : 0
        branchPopup.isEnabled = isGitRepo
        newBranchBtn.isEnabled = isGitRepo
        fetchBtn.isEnabled = isGitRepo && hasRemote
        pullBtn.isEnabled = isGitRepo && hasRemote
        pushBtn.isEnabled = isGitRepo && hasRemote
        syncBtn.isEnabled = isGitRepo && hasRemote
        stashBtn.isEnabled = isGitRepo && hasChanges
        popStashBtn.isEnabled = isGitRepo
        generateBtn.isEnabled = isGitRepo && hasChanges
        commitBtn.isEnabled = isGitRepo && (hasStaged || hasChanges)
        commitPushBtn.isEnabled = isGitRepo && hasRemote && (hasStaged || hasChanges)
        amendBtn.isEnabled = isGitRepo
        stageAllBtn.isEnabled = isGitRepo && hasChanges
        unstageAllBtn.isEnabled = isGitRepo && hasStaged
        pushBtn.title = aheadCount > 0 ? "Push ↑\(aheadCount)" : "Push"
        pullBtn.title = behindCount > 0 ? "Pull ↓\(behindCount)" : "Pull"
        openGitHubBtn.isHidden = !(isGitRepo && hasRemote && !remoteWebURL.isEmpty)
        if !remoteDisplayName.isEmpty {
            openGitHubBtn.title = remoteDisplayName
            openGitHubBtn.toolTip = "Open \(remoteWebURL) in your browser"
        }
        syncStatusLabel.stringValue = isGitRepo ? (hasRemote ? "↑\(aheadCount) ↓\(behindCount)" : "Local only") : "Unversioned"
    }

    private func loadGitHubStatus() {
        request("git.githubStatus", [:]) { [weak self] result in
            guard let self, case .success(let data) = result else { return }
            let status = GitHubRemoteStatus.parse(data)
            self.remoteWebURL = status.webUrl
            self.remoteDisplayName = status.remoteName
            self.remoteBanner.update(status: status)
            self.openGitHubBtn.isHidden = !(self.isGitRepo && self.hasRemote && !status.webUrl.isEmpty)
            if !status.remoteName.isEmpty {
                self.openGitHubBtn.title = status.remoteName
                self.openGitHubBtn.toolTip = "Open \(status.webUrl) in your browser"
            }
        }
    }

    private func loadBranches() {
        request("git.branchList", [:]) { [weak self] result in
            guard let self, case .success(let data) = result else { return }
            let branches = data["branches"] as? [String] ?? [self.currentBranch]
            let current = data["current"] as? String ?? self.currentBranch
            self.branchPopup.removeAllItems()
            self.branchPopup.addItems(withTitles: branches.isEmpty ? [current] : branches)
            self.branchPopup.selectItem(withTitle: current)
        }
    }

    private func loadGraph() {
        request("git.log", [:]) { [weak self] result in
            guard let self, case .success(let data) = result else { return }
            let commits = (data["commits"] as? [[String: Any]] ?? []).compactMap { item -> GitGraphCommit? in
                guard let hash = item["hash"] as? String else { return nil }
                return GitGraphCommit(
                    hash: hash,
                    shortHash: item["shortHash"] as? String ?? String(hash.prefix(7)),
                    subject: item["subject"] as? String ?? "",
                    author: item["author"] as? String ?? "",
                    relativeTime: item["relativeTime"] as? String ?? "",
                    refs: item["refs"] as? [String] ?? [],
                    isHead: item["isHead"] as? Bool == true
                )
            }
            self.graphView.update(commits: commits, repoName: self.folder.lastPathComponent, hasRemote: self.hasRemote)
        }
    }

    private func runGit(_ method: String, params: [String: Any], notice: String? = nil, onDone: (() -> Void)? = nil) {
        request(method, params) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let data):
                if data["ok"] as? Bool == false {
                    self.noticeLabel.stringValue = data["error"] as? String ?? "Git operation failed."
                    return
                }
                if let notice { self.noticeLabel.stringValue = notice }
                onDone?()
                self.refreshAll()
            case .failure(let err):
                self.noticeLabel.stringValue = err.localizedDescription
            }
        }
    }

    private func generateCommitMessage() {
        request("git.generateCommitMessage", [:]) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let data):
                if let msg = data["message"] as? String {
                    self.commitField.stringValue = msg
                    self.noticeLabel.stringValue = "Generated commit message."
                } else {
                    self.noticeLabel.stringValue = data["error"] as? String ?? "Could not generate commit message."
                }
            case .failure(let err): self.noticeLabel.stringValue = err.localizedDescription
            }
        }
    }

    private func commit(push: Bool, amend: Bool) {
        let msg = commitField.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !msg.isEmpty else { noticeLabel.stringValue = "Enter a commit message first."; return }
        runGit("git.commit", params: ["message": msg, "autoStage": true, "push": push, "amend": amend], notice: push ? "Committed and pushed." : "Committed changes.") { [weak self] in
            self?.commitField.stringValue = ""
        }
    }

    @objc private func switchBranch() {
        guard let target = branchPopup.titleOfSelectedItem, target != currentBranch else { return }
        runGit("git.checkout", params: ["target": target], notice: "Switched to \(target).")
    }

    private func promptNewBranch() {
        guard let window else { return }
        let alert = NSAlert()
        alert.messageText = "Create New Branch"
        let input = NSTextField(frame: NSRect(x: 0, y: 0, width: 240, height: 24))
        input.placeholderString = "feature/my-branch"
        alert.accessoryView = input
        alert.addButton(withTitle: "Create")
        alert.addButton(withTitle: "Cancel")
        alert.beginSheetModal(for: window) { [weak self] response in
            guard response == .alertFirstButtonReturn else { return }
            let name = input.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !name.isEmpty else { return }
            self?.runGit("git.checkout", params: ["target": name, "create": true], notice: "Created and switched to \(name).")
        }
    }

    @objc private func diffScopeChanged() {
        guard let path = selectedPath else { return }
        loadFileDiff(path, staged: diffMode.selectedSegment == 1)
    }

    private func loadFileDiff(_ path: String, staged: Bool) {
        selectedPath = path; selectedStaged = staged
        diffMode.selectedSegment = staged ? 1 : 0
        diffTitleLabel.stringValue = "\(path) (\(staged ? "Staged" : "Working Tree"))"
        request("changes.read", ["path": path, "staged": staged, "offset": 0, "limit": 65536]) { [weak self] result in
            guard let self else { return }
            if case .success(let data) = result {
                let text = data["text"] as? String ?? ""
                self.diffView.textStorage?.setAttributedString(CodeOutline.highlight(text.isEmpty ? "No diff available." : text, diff: true, fontSize: 12))
            }
        }
    }

    private func loadCommitDiff(_ hash: String) {
        diffTitleLabel.stringValue = "Commit \(String(hash.prefix(7)))"
        request("git.show", ["hash": hash, "offset": 0, "limit": 65536]) { [weak self] result in
            guard let self else { return }
            if case .success(let data) = result {
                let text = data["text"] as? String ?? ""
                self.diffView.textStorage?.setAttributedString(CodeOutline.highlight(text.isEmpty ? "No diff for commit." : text, diff: true, fontSize: 12))
            }
        }
    }

    func numberOfRows(in tableView: NSTableView) -> Int { items.count }

    func tableViewSelectionDidChange(_ notification: Notification) {
        let row = table.selectedRow
        guard items.indices.contains(row), !items[row].isSectionHeader else { return }
        let item = items[row]
        loadFileDiff(item.path, staged: item.staged)
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard items.indices.contains(row) else { return nil }
        let item = items[row]
        let cell = NSTableCellView()
        if item.isSectionHeader {
            let header = NSTextField(labelWithString: "\(item.title) (\(item.sectionCount))")
            header.font = .systemFont(ofSize: 10.5, weight: .bold)
            header.textColor = .secondaryLabelColor
            header.translatesAutoresizingMaskIntoConstraints = false
            cell.addSubview(header)
            NSLayoutConstraint.activate([header.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 6), header.centerYAnchor.constraint(equalTo: cell.centerYAnchor)])
            return cell
        }
        let name = NSTextField(labelWithString: item.title)
        name.font = .systemFont(ofSize: 12); name.lineBreakMode = .byTruncatingMiddle
        name.setContentHuggingPriority(.init(1), for: .horizontal)
        let badge = NSTextField(labelWithString: item.statusBadge)
        badge.font = .monospacedSystemFont(ofSize: 11, weight: .bold)
        badge.textColor = item.statusBadge == "U" || item.statusBadge == "A" ? .systemGreen : item.statusBadge == "D" ? .systemRed : .systemOrange
        var views: [NSView] = [name, badge]
        if item.staged {
            let unstage = JourneyButton("", symbol: "minus.circle") { [weak self] in self?.runGit("git.unstage", params: ["path": item.path]) }
            unstage.bezelStyle = .inline; unstage.isBordered = false; unstage.toolTip = "Unstage"
            views.append(unstage)
        } else {
            if !item.untracked {
                let discard = JourneyButton("", symbol: "arrow.uturn.backward") { [weak self] in
                    guard let self, let window = self.window else { return }
                    let alert = NSAlert()
                    alert.messageText = "Discard changes to \(item.title)?"
                    alert.informativeText = "This will revert unstaged edits to \(item.path) back to HEAD."
                    alert.addButton(withTitle: "Cancel")
                    alert.addButton(withTitle: "Discard").hasDestructiveAction = true
                    alert.beginSheetModal(for: window) { response in
                        guard response == .alertSecondButtonReturn else { return }
                        self.runGit("git.discard", params: ["path": item.path], notice: "Discarded \(item.title).")
                    }
                }
                discard.bezelStyle = .inline; discard.isBordered = false; discard.toolTip = "Discard changes"
                views.append(discard)
            }
            let stage = JourneyButton("", symbol: "plus.circle") { [weak self] in self?.runGit("git.stage", params: ["path": item.path]) }
            stage.bezelStyle = .inline; stage.isBordered = false; stage.toolTip = "Stage"
            views.append(stage)
        }
        let rowStack = NSStackView(views: views)
        rowStack.orientation = .horizontal; rowStack.alignment = .centerY; rowStack.spacing = 6
        rowStack.translatesAutoresizingMaskIntoConstraints = false
        cell.addSubview(rowStack); cell.toolTip = item.path
        NSLayoutConstraint.activate([
            rowStack.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 6),
            rowStack.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -6),
            rowStack.centerYAnchor.constraint(equalTo: cell.centerYAnchor)
        ])
        return cell
    }

    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        if commandSelector == #selector(NSResponder.insertNewline(_:)),
           NSApp.currentEvent?.modifierFlags.contains(.command) == true {
            commit(push: false, amend: false)
            return true
        }
        return false
    }

    func windowWillClose(_ notification: Notification) {
        dispose()
        onClose?()
        onClose = nil
    }

    func dispose() {
        remoteBanner.dispose()
        graphView.dispose()
    }
}
