// [LAYER: PLUMBING]
import AppKit

struct ConversationSummary {
    let id: String
    let title: String
    let detail: String
}

final class ConversationSidebar: NSVisualEffectView, NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate {
    var onNew: (() -> Void)?
    var onDelete: ((String) -> Void)?
    var supportsDeletion = false
    private var deleteButtons = [NSButton]()
    var onSelect: ((String) -> Void)?
    var onProjects: (() -> Void)?
    var onAccount: (() -> Void)?
    var onMore: (() -> Void)?
    var onRecent: (() -> Void)?
    var onItemMenu: ((ConversationSummary, NSView) -> Void)?
    var onWorkspaceMenu: ((NSView) -> Void)?
    private let table = NSTableView()
    private let search = NSSearchField()
    private let message = JourneyStyle.label("Connecting to LUMI…", size: 11, secondary: true)
    private let title = JourneyStyle.label("Workspace", size: 14, weight: .semibold)
    private var all = [ConversationSummary]()
    private var visible = [ConversationSummary]()
    private var selected: String?
    private var navigationMessage = ""
    private var hasEarlierConversations = false
    private var suppress = false
    private lazy var create = JourneyButton("New conversation", symbol: "plus.bubble") { [weak self] in self?.onNew?() }
    private lazy var workspaceMenu: JourneyButton = JourneyButton("Workspace Menu…", symbol: "ellipsis.circle") { [weak self] in
        guard let self else { return }
        self.onWorkspaceMenu?(self.workspaceMenu)
    }
    private lazy var more = JourneyButton("Earlier conversations") { [weak self] in self?.onMore?() }
    private lazy var recent = JourneyButton("Latest conversations") { [weak self] in self?.onRecent?() }

    init(folder: URL) {
        super.init(frame: .zero)
        material = .sidebar; blendingMode = .withinWindow; state = .followsWindowActiveState
        title.stringValue = folder.lastPathComponent; title.maximumNumberOfLines = 1; title.lineBreakMode = .byTruncatingMiddle
        title.toolTip = folder.path
        let icon = NSImageView(image: NSImage(systemSymbolName: "folder", accessibilityDescription: nil) ?? NSImage())
        icon.contentTintColor = .controlAccentColor; icon.widthAnchor.constraint(equalToConstant: 20).isActive = true
        let heading = JourneyStyle.stack([icon, title], vertical: false, spacing: 10)
        search.placeholderString = "Find on this page"; search.delegate = self
        search.setAccessibilityLabel("Find a loaded conversation")
        table.addTableColumn(NSTableColumn(identifier: .init("conversation")))
        table.headerView = nil; table.dataSource = self; table.delegate = self
        table.style = .sourceList; table.backgroundColor = .clear; table.rowHeight = 54
        table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        table.setAccessibilityLabel("Conversations")
        let scroll = NSScrollView(); scroll.documentView = table; scroll.hasVerticalScroller = true; scroll.drawsBackground = false
        scroll.setContentHuggingPriority(.init(1), for: .vertical)
        let projects = JourneyButton("Project Library…", symbol: "folder") { [weak self] in self?.onProjects?() }
        projects.toolTip = "Browse or open saved projects (⌘0)"
        let account = JourneyButton("Account", symbol: "person.crop.circle") { [weak self] in self?.onAccount?() }
        workspaceMenu.toolTip = "Open all workspace features, panels, and navigation options"
        for button in [create, workspaceMenu, projects, account] { button.alignment = .left; button.bezelStyle = .inline; button.heightAnchor.constraint(equalToConstant: 32).isActive = true }
        let root = JourneyStyle.stack([heading, JourneyStyle.rule(), create, JourneyStyle.label("Conversations", size: 11, weight: .semibold, secondary: true), search, scroll, message, more, recent, JourneyStyle.rule(), workspaceMenu, projects, account], spacing: 12)
        addSubview(root); JourneyStyle.pin(root, to: self, inset: 14)
        for view in root.arrangedSubviews { view.widthAnchor.constraint(equalTo: root.widthAnchor).isActive = true }
        more.isHidden = true; recent.isHidden = true; create.isEnabled = false
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }
    func show(_ conversations: [ConversationSummary], selected: String?, canSwitch: Bool, more: Bool, earlierPage: Bool = false, message: String = "") {
        all = conversations; self.selected = selected
        hasEarlierConversations = more
        create.isEnabled = canSwitch; table.isEnabled = canSwitch
        create.toolTip = canSwitch ? "Start with a fresh conversation" : "Finish or stop current work before switching conversations"
        self.more.isHidden = !more; self.more.isEnabled = canSwitch
        recent.isHidden = !earlierPage; recent.isEnabled = canSwitch
        navigationMessage = message
        render()
    }
    func setCanSwitch(_ allowed: Bool) {
        deleteButtons.forEach { $0.isEnabled = allowed }
        create.isEnabled = allowed; table.isEnabled = allowed; more.isEnabled = allowed; recent.isEnabled = allowed
        create.toolTip = allowed ? "Start with a fresh conversation" : "Finish or stop current work before switching conversations"
    }
    private func render() {
        let query = search.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        visible = all.filter { query.isEmpty || $0.title.localizedCaseInsensitiveContains(query) }
        deleteButtons.removeAll()
        suppress = true; table.deselectAll(nil); table.reloadData()
        if let index = visible.firstIndex(where: { $0.id == selected }) { table.selectRowIndexes(IndexSet(integer: index), byExtendingSelection: false) }
        suppress = false
        if visible.isEmpty && !query.isEmpty && navigationMessage.isEmpty {
            message.stringValue = hasEarlierConversations
                ? "No match on this page. Choose Earlier conversations to keep searching."
                : "No matching conversations on this page."
        } else {
            message.stringValue = navigationMessage
        }
        message.isHidden = message.stringValue.isEmpty
    }
    func controlTextDidChange(_ obj: Notification) { render() }
    func numberOfRows(in tableView: NSTableView) -> Int { visible.count }
    func tableViewSelectionDidChange(_ notification: Notification) {
        guard table.isEnabled, !suppress, visible.indices.contains(table.selectedRow) else { return }
        let id = visible[table.selectedRow].id
        if id != selected { onSelect?(id) }
    }
    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard visible.indices.contains(row) else { return nil }
        let item = visible[row], cell = NSTableCellView()
        let name = JourneyStyle.label(item.title, size: 13, weight: item.id == selected ? .medium : .regular)
        name.maximumNumberOfLines = 1; name.lineBreakMode = .byTruncatingTail
        let detail = JourneyStyle.label(item.detail, size: 11, secondary: true); detail.maximumNumberOfLines = 1
        let labels = JourneyStyle.stack([name, detail], spacing: 4)
        let icon = NSImageView(image: NSImage(systemSymbolName: "bubble.left", accessibilityDescription: nil) ?? NSImage())
        icon.widthAnchor.constraint(equalToConstant: 18).isActive = true
        let menuBtn = JourneyButton("", symbol: "ellipsis.circle") { [weak self] in
            guard let self else { return }
            let anchor = self.table.view(atColumn: 0, row: row, makeIfNecessary: false) ?? self.table
            self.onItemMenu?(item, anchor)
        }
        menuBtn.bezelStyle = .inline; menuBtn.isBordered = false
        menuBtn.toolTip = "Conversation options…"
        menuBtn.setAccessibilityLabel("Options for \(item.title)")
        menuBtn.widthAnchor.constraint(equalToConstant: 24).isActive = true
        menuBtn.heightAnchor.constraint(equalToConstant: 24).isActive = true
        let delete = JourneyButton("", symbol: "trash") { [weak self] in
            guard let self, self.table.isEnabled else { return }
            self.onDelete?(item.id)
        }
        delete.bezelStyle = .inline
        delete.toolTip = "Delete conversation…"
        delete.setAccessibilityLabel("Delete conversation: \(item.title)")
        delete.isEnabled = table.isEnabled
        delete.isHidden = !supportsDeletion
        delete.widthAnchor.constraint(equalToConstant: 28).isActive = true
        delete.heightAnchor.constraint(equalToConstant: 28).isActive = true
        deleteButtons.append(delete)
        labels.setContentCompressionResistancePriority(.init(1), for: .horizontal)
        let root = JourneyStyle.stack([icon, labels, delete, menuBtn], vertical: false, spacing: 6)
        cell.addSubview(root); JourneyStyle.pin(root, to: cell, inset: 6)
        name.widthAnchor.constraint(equalTo: labels.widthAnchor).isActive = true
        detail.widthAnchor.constraint(equalTo: labels.widthAnchor).isActive = true
        cell.textField = name; cell.imageView = icon; cell.toolTip = item.title
        return cell
    }
    func dispose() {
        onSelect = nil; onDelete = nil; onNew = nil; onMore = nil
        onRecent = nil; onProjects = nil; onAccount = nil
        onItemMenu = nil; onWorkspaceMenu = nil
        deleteButtons.removeAll()
    }
}
