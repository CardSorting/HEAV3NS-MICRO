// [LAYER: PLUMBING]
import AppKit

enum WorkspaceDestination: Int, CaseIterable {
    case files, changes, sessions
    var title: String { ["Files", "Source Control", "Recorded Runs"][rawValue] }
    var symbol: String { ["folder", "arrow.triangle.branch", "clock"][rawValue] }
}

struct SidebarEntry {
    let id: String
    let title: String
    let detail: String
    let symbol: String
    let value: [String: Any]
}

/// Native selection and keyboard behavior; only the requested page is loaded.
final class NavigationSidebar: NSView, NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate {
    var onDestination: ((WorkspaceDestination) -> Void)?
    var onOpen: ((SidebarEntry) -> Void)?
    var onFilter: ((String) -> Void)?
    var onBack: (() -> Void)?
    var onNext: (() -> Void)?
    var onPrevious: (() -> Void)?
    var onRefresh: (() -> Void)?
    var onItemMenu: ((SidebarEntry, NSView) -> Void)?
    var buildItemMenu: ((SidebarEntry) -> NSMenu?)?
    var onWorkspaceMenu: ((NSView) -> Void)?
    var onInitGit: (() -> Void)?
    var onOpenSourceControl: (() -> Void)?
    private(set) var destination = WorkspaceDestination.files
    private(set) var entries = [SidebarEntry]()
    private let destinations = NSTableView()
    private let items = KeyboardListView()
    private let search = NSSearchField()
    private let location = NSTextField(labelWithString: "Workspace")
    private let message = NSTextField(wrappingLabelWithString: "Choose Files to browse this workspace.")
    private let back = NSButton(image: NSImage(systemSymbolName: "chevron.up", accessibilityDescription: "Parent folder") ?? NSImage(), target: nil, action: nil)
    private let next = NSButton(title: "Next 100", target: nil, action: nil)
    private let previous = NSButton(image: NSImage(systemSymbolName: "chevron.left", accessibilityDescription: "Previous page") ?? NSImage(), target: nil, action: nil)
    private var suppressSelection = false
    private var filterWork: DispatchWorkItem?
    private let destinationScroll = NSScrollView()
    private let destinationRule = NSBox()
    private let root = NSStackView()
    private let sourceControlBar = NSStackView()
    private let initGitRow = NSStackView()
    private let openSourceControlRow = NSStackView()
    private let gitBranchLabel = NSTextField(labelWithString: "main")
    private var compact = false
    private var optionsButton: NSButton!
    var previewsSelection = true

    func useAsContextBrowser() {
        compact = true; destinationScroll.isHidden = true; destinationRule.isHidden = true
        root.spacing = 6; root.edgeInsets = NSEdgeInsets(top: 8, left: 8, bottom: 8, right: 8)
        items.rowHeight = 32
    }

    override init(frame: NSRect) {
        super.init(frame: frame)
        let background = NSVisualEffectView(); background.material = .sidebar; background.blendingMode = .withinWindow
        background.autoresizingMask = [.width, .height]; background.frame = bounds; addSubview(background)
        root.orientation = .vertical; root.alignment = .leading; root.spacing = 10; root.distribution = .fill
        root.edgeInsets = NSEdgeInsets(top: 12, left: 8, bottom: 12, right: 8)
        for table in [destinations, items] {
            table.addTableColumn(NSTableColumn(identifier: NSUserInterfaceItemIdentifier("name")))
            table.headerView = nil; table.dataSource = self; table.delegate = self
            table.style = .sourceList; table.allowsMultipleSelection = false
            table.backgroundColor = .clear
            table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
            table.rowHeight = table === destinations ? 30 : 42
        }
        destinations.setAccessibilityLabel("Workspace navigation")
        items.setAccessibilityLabel("Workspace items")
        items.toolTip = "Arrow keys select. Return or double-click opens. Right opens a folder; Left goes to its parent."
        items.target = self; items.doubleAction = #selector(activateSelected)
        items.onActivate = { [weak self] in self?.activateSelected() }
        items.onEnterFolder = { [weak self] in
            guard let self, self.entries.indices.contains(self.items.selectedRow), self.entries[self.items.selectedRow].value["directory"] as? Bool == true else { return }
            self.activateSelected()
        }
        items.onParent = { [weak self] in guard let self, self.destination == .files, self.back.isEnabled else { return }; self.onBack?() }
        items.contextMenuForRow = { [weak self] row in
            guard let self, self.entries.indices.contains(row) else { return nil }
            return self.buildItemMenu?(self.entries[row])
        }
        destinationScroll.documentView = destinations; destinationScroll.drawsBackground = false
        destinationScroll.heightAnchor.constraint(equalToConstant: 94).isActive = true
        root.addArrangedSubview(destinationScroll)
        let rule = destinationRule; rule.boxType = .separator; root.addArrangedSubview(rule)
        let pathBar = NSStackView(views: [back, location]); pathBar.spacing = 5
        location.font = .systemFont(ofSize: 11, weight: .semibold); location.lineBreakMode = .byTruncatingMiddle
        location.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        back.bezelStyle = .texturedRounded; back.target = self; back.action = #selector(parent)
        back.setAccessibilityLabel("Parent folder"); back.toolTip = "Open parent folder"
        root.addArrangedSubview(pathBar)
        search.placeholderString = "Filter this folder"; search.delegate = self; search.setAccessibilityLabel("Filter workspace items")
        root.addArrangedSubview(search)

        sourceControlBar.orientation = .vertical; sourceControlBar.alignment = .leading; sourceControlBar.spacing = 6
        let initLabel = NSTextField(labelWithString: "Not a Git repository")
        initLabel.font = .systemFont(ofSize: 11); initLabel.textColor = .secondaryLabelColor
        initLabel.setContentHuggingPriority(.init(1), for: .horizontal)
        let initBtn = JourneyButton("Initialize Git", symbol: "arrow.triangle.branch") { [weak self] in self?.onInitGit?() }
        initBtn.controlSize = .small; initBtn.bezelStyle = .rounded
        initGitRow.orientation = .horizontal; initGitRow.alignment = .centerY; initGitRow.spacing = 6
        [initLabel, initBtn].forEach(initGitRow.addArrangedSubview)

        let openSCBtn = JourneyButton("Open Source Control…", symbol: "arrow.triangle.branch") { [weak self] in self?.onOpenSourceControl?() }
        openSCBtn.controlSize = .small; openSCBtn.bezelStyle = .rounded; openSCBtn.bezelColor = .controlAccentColor
        gitBranchLabel.font = .systemFont(ofSize: 11, weight: .semibold); gitBranchLabel.lineBreakMode = .byTruncatingMiddle
        gitBranchLabel.setContentHuggingPriority(.init(1), for: .horizontal)
        openSourceControlRow.orientation = .horizontal; openSourceControlRow.alignment = .centerY; openSourceControlRow.spacing = 6
        [gitBranchLabel, openSCBtn].forEach(openSourceControlRow.addArrangedSubview)

        [initGitRow, openSourceControlRow].forEach(sourceControlBar.addArrangedSubview)
        sourceControlBar.isHidden = true
        root.addArrangedSubview(sourceControlBar)

        let scroll = NSScrollView(); scroll.documentView = items; scroll.hasVerticalScroller = true; scroll.drawsBackground = false
        scroll.setContentHuggingPriority(.init(1), for: .vertical); root.addArrangedSubview(scroll)
        message.font = .systemFont(ofSize: 11); message.textColor = .secondaryLabelColor; root.addArrangedSubview(message)
        let refresh = NSButton(image: NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: "Refresh") ?? NSImage(), target: self, action: #selector(reload))
        refresh.bezelStyle = .texturedRounded; refresh.setAccessibilityLabel("Refresh current destination")
        next.target = self; next.action = #selector(nextPage); next.bezelStyle = .rounded; next.isHidden = true
        previous.target = self; previous.action = #selector(previousPage); previous.bezelStyle = .texturedRounded; previous.isEnabled = false
        previous.setAccessibilityLabel("Previous page")
        let options = JourneyButton("", symbol: "ellipsis.circle") { [weak self] in
            guard let self else { return }
            self.onWorkspaceMenu?(self.optionsButton)
        }
        options.bezelStyle = .texturedRounded
        options.setAccessibilityLabel("Sidebar workspace options")
        options.toolTip = "Open workspace and navigation menu"
        optionsButton = options
        let footer = NSStackView(views: [refresh, previous, next, options]); footer.spacing = 6; root.addArrangedSubview(footer)
        addSubview(root); root.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([root.leadingAnchor.constraint(equalTo: leadingAnchor), root.trailingAnchor.constraint(equalTo: trailingAnchor), root.topAnchor.constraint(equalTo: topAnchor), root.bottomAnchor.constraint(equalTo: bottomAnchor)])
        for child in [destinationScroll, rule, pathBar, search, sourceControlBar, scroll, message, footer] { child.widthAnchor.constraint(equalTo: root.widthAnchor, constant: -16).isActive = true }
        for row in [initGitRow, openSourceControlRow] { row.widthAnchor.constraint(equalTo: sourceControlBar.widthAnchor).isActive = true }
        select(.files, notify: false)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }
    func select(_ value: WorkspaceDestination, notify: Bool = true) {
        resetFilter(); destination = value
        suppressSelection = true; destinations.selectRowIndexes(IndexSet(integer: value.rawValue), byExtendingSelection: false); suppressSelection = false
        search.placeholderString = value == .files ? "Filter this folder" : value == .sessions ? "Filter runs on this page" : "Filter changed files"
        sourceControlBar.isHidden = value != .changes
        items.rowHeight = value == .changes ? 24 : (compact ? 32 : 42)
        if notify { onDestination?(value) }
    }

    func setSourceControlState(isGitRepo: Bool, branch: String, hasChanges: Bool) {
        sourceControlBar.isHidden = destination != .changes
        initGitRow.isHidden = isGitRepo
        openSourceControlRow.isHidden = false
        gitBranchLabel.stringValue = isGitRepo ? (hasChanges ? "\(branch)*" : branch) : "Not Git"
    }
    func show(_ entries: [SidebarEntry], location: String, message: String, canGoBack: Bool = false, more: Bool = false, hasPrevious: Bool = false) {
        let selected = items.selectedRow >= 0 && self.entries.indices.contains(items.selectedRow) ? self.entries[items.selectedRow].id : nil
        self.entries = entries; self.location.stringValue = location; self.location.toolTip = location; items.isEnabled = true
        self.message.stringValue = message + (!previewsSelection && !entries.isEmpty ? " · Return or double-click to open" : "")
        self.message.isHidden = self.message.stringValue.isEmpty
        back.isEnabled = canGoBack; back.isHidden = destination != .files
        next.isHidden = !more; next.title = destination == .sessions ? "Earlier Runs" : "Next 100"
        previous.isEnabled = hasPrevious
        suppressSelection = true; items.deselectAll(nil); items.reloadData()
        if let selected, let index = entries.firstIndex(where: { $0.id == selected }) { items.selectRowIndexes(IndexSet(integer: index), byExtendingSelection: false) }
        suppressSelection = false
    }
    func loading(_ text: String) { message.stringValue = text; message.isHidden = false; next.isHidden = true; items.isEnabled = false; previous.isEnabled = false }
    func focusFilter() { window?.makeFirstResponder(search) }
    func resetFilter() { filterWork?.cancel(); filterWork = nil; search.stringValue = "" }
    func controlTextDidChange(_ obj: Notification) {
        filterWork?.cancel()
        let query = search.stringValue
        let work = DispatchWorkItem { [weak self] in self?.onFilter?(query) }; filterWork = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.18, execute: work)
    }
    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        guard !textView.hasMarkedText() else { return false }
        if commandSelector == #selector(NSResponder.moveDown(_:)), !entries.isEmpty, items.isEnabled {
            window?.makeFirstResponder(items); items.selectRowIndexes(IndexSet(integer: max(0, items.selectedRow)), byExtendingSelection: false); return true
        }
        return false
    }
    @objc private func activateSelected() {
        guard items.isEnabled, entries.indices.contains(items.selectedRow) else { return }
        onOpen?(entries[items.selectedRow])
    }
    @objc private func parent() { onBack?() }
    @objc private func nextPage() { onNext?() }
    @objc private func previousPage() { onPrevious?() }
    @objc private func reload() { onRefresh?() }
    func numberOfRows(in tableView: NSTableView) -> Int { tableView === destinations ? 3 : entries.count }
    func tableViewSelectionDidChange(_ notification: Notification) {
        guard !suppressSelection, let table = notification.object as? NSTableView else { return }
        if table === destinations, let value = WorkspaceDestination(rawValue: table.selectedRow) { select(value) }
        else if table === items, previewsSelection, items.isEnabled, entries.indices.contains(table.selectedRow),
                entries[table.selectedRow].value["isSectionHeader"] as? Bool != true,
                entries[table.selectedRow].value["directory"] as? Bool != true { onOpen?(entries[table.selectedRow]) }
    }
    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let entry: SidebarEntry
        if tableView === destinations {
            guard WorkspaceDestination.allCases.indices.contains(row) else { return nil }
            let dest = WorkspaceDestination.allCases[row]
            entry = SidebarEntry(id: "", title: dest.title, detail: "", symbol: dest.symbol, value: [:])
        } else {
            guard entries.indices.contains(row) else { return nil }
            entry = entries[row]
        }
        let cell = NSTableCellView()
        if entry.value["isSectionHeader"] as? Bool == true {
            let header = NSTextField(labelWithString: entry.title)
            header.font = .systemFont(ofSize: 10.5, weight: .bold)
            header.textColor = .secondaryLabelColor
            header.translatesAutoresizingMaskIntoConstraints = false
            cell.addSubview(header)
            var views: [NSView] = [header]
            if let count = entry.value["sectionCount"] as? Int {
                let badge = NSTextField(labelWithString: " \(count) ")
                badge.font = .systemFont(ofSize: 10, weight: .semibold)
                badge.textColor = .secondaryLabelColor
                badge.wantsLayer = true
                badge.layer?.cornerRadius = 6
                badge.layer?.backgroundColor = NSColor.labelColor.withAlphaComponent(0.08).cgColor
                badge.setContentHuggingPriority(.required, for: .horizontal)
                views.append(badge)
            }
            let bar = NSStackView(views: views)
            bar.orientation = .horizontal; bar.alignment = .centerY; bar.spacing = 4; bar.translatesAutoresizingMaskIntoConstraints = false
            header.setContentHuggingPriority(.init(1), for: .horizontal)
            cell.addSubview(bar)
            NSLayoutConstraint.activate([bar.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 6), bar.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -6), bar.centerYAnchor.constraint(equalTo: cell.centerYAnchor)])
            return cell
        }
        let icon = NSImageView(image: NSImage(systemSymbolName: entry.symbol, accessibilityDescription: nil) ?? NSImage())
        icon.contentTintColor = destination == .changes ? (entry.value["staged"] as? Bool == true && entry.value["section"] as? String == "staged" ? .systemGreen : .systemOrange) : .controlAccentColor
        let title = NSTextField(labelWithString: entry.title); title.font = .systemFont(ofSize: 12); title.lineBreakMode = .byTruncatingMiddle
        let detail = NSTextField(labelWithString: entry.detail); detail.font = .systemFont(ofSize: 10); detail.textColor = .secondaryLabelColor; detail.lineBreakMode = .byTruncatingMiddle
        let showDetail = !entry.detail.isEmpty && destination != .changes && (!compact || destination != .files)
        let labels = NSStackView(views: showDetail ? [title, detail] : [title]); labels.orientation = .vertical; labels.alignment = .leading; labels.spacing = 2
        var rowViews: [NSView] = [icon, labels]
        if tableView === items {
            if destination == .changes, let badgeChar = entry.value["statusBadge"] as? String {
                let statusLabel = NSTextField(labelWithString: badgeChar)
                statusLabel.font = .monospacedSystemFont(ofSize: 10.5, weight: .bold)
                statusLabel.textColor = badgeChar == "U" || badgeChar == "A" ? .systemGreen : badgeChar == "D" ? .systemRed : .systemOrange
                statusLabel.setContentHuggingPriority(.required, for: .horizontal)
                rowViews.append(statusLabel)
            }
            let itemMenu = JourneyButton("", symbol: "ellipsis.circle") { [weak self] in
                guard let self, let anchor = self.items.view(atColumn: 0, row: row, makeIfNecessary: false) else { return }
                self.onItemMenu?(entry, anchor)
            }
            itemMenu.bezelStyle = .inline; itemMenu.isBordered = false
            itemMenu.setAccessibilityLabel("Options for \(entry.title)")
            itemMenu.toolTip = "Show actions for \(entry.title)"
            itemMenu.widthAnchor.constraint(equalToConstant: 20).isActive = true
            rowViews.append(itemMenu)
        }
        let content = NSStackView(views: rowViews); content.spacing = 6; content.translatesAutoresizingMaskIntoConstraints = false
        cell.addSubview(content); cell.textField = title; cell.imageView = icon; cell.toolTip = entry.title + (entry.detail.isEmpty ? "" : "\n" + entry.detail)
        NSLayoutConstraint.activate([content.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 5), content.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -5), content.centerYAnchor.constraint(equalTo: cell.centerYAnchor), icon.widthAnchor.constraint(equalToConstant: 16), title.widthAnchor.constraint(equalTo: labels.widthAnchor)])
        if showDetail { detail.widthAnchor.constraint(equalTo: labels.widthAnchor).isActive = true }
        return cell
    }
    func dispose() {
        filterWork?.cancel()
        onDestination = nil; onOpen = nil; onFilter = nil; onBack = nil
        onNext = nil; onPrevious = nil; onRefresh = nil; onItemMenu = nil; buildItemMenu = nil; onWorkspaceMenu = nil
        onInitGit = nil; onOpenSourceControl = nil
        items.contextMenuForRow = nil
    }
    deinit { filterWork?.cancel() }
}
