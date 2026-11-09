// [LAYER: UI]
import AppKit

/// At most one bridge read is outstanding. Typing replaces the queued query;
/// old responses never become selectable results for a newer query.
final class FileSearchRequestQueue {
    typealias Search = (String, Bool, @escaping (Result<[String: Any], Error>) -> Void) -> Void
    private let search: Search
    var onResult: ((Result<[String: Any], Error>) -> Void)?
    private var generation = 0
    private var pending: (String, Bool, Int)?
    private var running = false
    private var closed = false
    init(search: @escaping Search) { self.search = search }
    func submit(_ query: String, refresh: Bool = false) {
        guard !closed else { return }
        generation += 1; pending = (query, refresh || pending?.1 == true, generation); drain()
    }
    func invalidate() { generation += 1; pending = nil }
    func close() { invalidate(); closed = true }
    private func drain() {
        guard !closed, !running, let (query, refresh, ticket) = pending else { return }
        pending = nil; running = true
        search(query, refresh) { [weak self] result in
            guard let self else { return }; self.running = false
            if !self.closed, ticket == self.generation { self.onResult?(result) }
            self.drain()
        }
    }
}

enum OmnibarItem {
    case file(path: String, name: String)
    case action(WorkspaceAction)
    case symbol(name: String, kind: String, line: Int, index: Int)
    case checkpoint(id: String, title: String, detail: String)
    case lineNumber(line: Int, column: Int, snippet: String)
}

struct OmnibarCheckpointItem: Equatable {
    let id: String
    let title: String
    let detail: String
}

enum OmnibarMode {
    case files
    case actions
    case symbols
    case checkpoints
    case line
}

/// A modeless native picker: the conversation and Stop remain available.
final class QuickOpenWindow: NSWindowController, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate, NSSearchFieldDelegate {
    private let search = NSSearchField()
    private let table = KeyboardListView()
    private let scope = JourneyStyle.label("Search filenames and paths in this workspace.", size: 12, secondary: true)
    private let status = JourneyStyle.label("Loading project files…", size: 12, secondary: true)
    private let chooseButton = NSButton(title: "Open Preview", target: nil, action: nil)
    private let retry = NSButton(title: "Refresh", target: nil, action: nil)
    private var items = [OmnibarItem]()
    private let actions: [WorkspaceAction]
    private let symbolsProvider: () -> [(name: String, kind: String, line: Int, index: Int)]
    private let checkpointsProvider: () -> [OmnibarCheckpointItem]
    private let linesProvider: () -> [String]
    var onSelectSymbol: ((Int) -> Void)?
    var onSelectCheckpoint: ((String) -> Void)?
    var onSelectLine: ((Int, Int) -> Void)?
    private let requests: FileSearchRequestQueue
    private let openFile: (String) -> Void
    private var debounce: DispatchWorkItem?
    private weak var previousResponder: NSResponder?
    private var dismissed = false

    init(folder: URL, search: @escaping FileSearchRequestQueue.Search, actions: [WorkspaceAction] = [],
         symbols: @escaping () -> [(name: String, kind: String, line: Int, index: Int)] = { [] },
         checkpoints: @escaping () -> [OmnibarCheckpointItem] = { [] },
         lines: @escaping () -> [String] = { [] },
         open: @escaping (String) -> Void) {
        self.requests = FileSearchRequestQueue(search: search)
        self.openFile = open
        self.actions = actions
        self.symbolsProvider = symbols
        self.checkpointsProvider = checkpoints
        self.linesProvider = lines
        let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 560, height: 370), styleMask: [.titled, .closable], backing: .buffered, defer: false)
        panel.title = "Omnibar — \(folder.lastPathComponent)"; panel.isReleasedWhenClosed = false
        panel.hidesOnDeactivate = false
        super.init(window: panel)
        panel.delegate = self
        self.search.placeholderString = "Search files (:line, >actions, @symbols, #checkpoints)"; self.search.delegate = self
        self.search.setAccessibilityLabel("Unified omnibar search")
        table.addTableColumn(NSTableColumn(identifier: .init("item")))
        table.headerView = nil; table.rowHeight = 46; table.dataSource = self; table.delegate = self; table.style = .plain
        table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        table.target = self; table.doubleAction = #selector(choose)
        table.setAccessibilityLabel("Matching items")
        table.onActivate = { [weak self] in self?.choose() }; table.onCancel = { [weak self] in self?.dismiss() }
        let scroll = NSScrollView(); scroll.documentView = table; scroll.hasVerticalScroller = true
        scroll.setContentHuggingPriority(.init(1), for: .vertical)
        chooseButton.target = self; chooseButton.action = #selector(choose); chooseButton.bezelStyle = .rounded; chooseButton.isEnabled = false
        retry.target = self; retry.action = #selector(refresh); retry.bezelStyle = .rounded
        let hint = JourneyStyle.label("↑ ↓ to select · Return to open · Esc to close · Type > commands, :line", size: 11, secondary: true)
        hint.maximumNumberOfLines = 1; hint.lineBreakMode = .byTruncatingTail
        let footer = JourneyStyle.stack([retry, NSView(), chooseButton], vertical: false, spacing: 8)
        let root = JourneyStyle.stack([scope, self.search, scroll, status, hint, footer], spacing: 10)
        if let content = panel.contentView {
            content.addSubview(root); JourneyStyle.pin(root, to: content, inset: 16)
            for view in root.arrangedSubviews { view.widthAnchor.constraint(equalTo: root.widthAnchor).isActive = true }
        }
        status.maximumNumberOfLines = 3
        requests.onResult = { [weak self] in self?.received($0) }
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }
    func present(for parent: NSWindow, mode: OmnibarMode = .files) {
        guard let window else { return }
        dismissed = false
        previousResponder = parent.firstResponder
        window.appearance = parent.appearance
        window.setFrameOrigin(NSPoint(x: parent.frame.midX - window.frame.width / 2, y: parent.frame.maxY - window.frame.height - 72))
        parent.addChildWindow(window, ordered: .above)
        window.makeKeyAndOrderFront(nil); window.makeFirstResponder(search)
        switch mode {
        case .files:
            search.stringValue = ""
            performSearch()
        case .actions:
            search.stringValue = ">"
            filterActions(query: "")
        case .symbols:
            search.stringValue = "@"
            filterSymbols(query: "")
        case .checkpoints:
            search.stringValue = "#"
            filterCheckpoints(query: "")
        case .line:
            search.stringValue = ":"
            filterLine(query: "")
        }
    }
    func dismiss(restoreFocus: Bool = true) {
        guard !dismissed else { return }; dismissed = true
        debounce?.cancel(); requests.close()
        let parent = window?.parent
        if let window { parent?.removeChildWindow(window); window.orderOut(nil) }
        if restoreFocus, let parent, parent.isVisible, NSApp.isActive {
            parent.makeKeyAndOrderFront(nil)
            if let previousResponder { parent.makeFirstResponder(previousResponder) }
        }
    }
    override func cancelOperation(_ sender: Any?) { dismiss() }
    func windowShouldClose(_ sender: NSWindow) -> Bool { dismiss(); return false }
    func windowDidResignKey(_ notification: Notification) {
        guard !dismissed else { return }
        dismiss(restoreFocus: false)
    }
    func controlTextDidChange(_ obj: Notification) {
        let text = search.stringValue
        if text.hasPrefix(">") {
            debounce?.cancel(); requests.invalidate()
            let query = String(text.dropFirst()).trimmingCharacters(in: .whitespaces)
            filterActions(query: query)
        } else if text.hasPrefix("@") {
            debounce?.cancel(); requests.invalidate()
            let query = String(text.dropFirst()).trimmingCharacters(in: .whitespaces)
            filterSymbols(query: query)
        } else if text.hasPrefix("#") {
            debounce?.cancel(); requests.invalidate()
            let query = String(text.dropFirst()).trimmingCharacters(in: .whitespaces)
            filterCheckpoints(query: query)
        } else if text.hasPrefix(":") {
            debounce?.cancel(); requests.invalidate()
            let query = String(text.dropFirst()).trimmingCharacters(in: .whitespaces)
            filterLine(query: query)
        } else {
            scope.stringValue = "Search filenames and paths in this workspace."
            chooseButton.title = "Open Preview"
            debounce?.cancel(); requests.invalidate(); clearResults("Searching filenames…")
            let work = DispatchWorkItem { [weak self] in self?.performSearch() }; debounce = work
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.18, execute: work)
        }
    }
    private func filterActions(query: String) {
        scope.stringValue = "Workspace Actions · Press Return to run command"
        chooseButton.title = "Run Command"
        let matches = actions.enumerated()
            .filter { query.isEmpty || $0.element.matches(query) }
            .sorted { a, b in
                let sa = a.element.matchScore(query)
                let sb = b.element.matchScore(query)
                return sa != sb ? sa > sb : a.offset < b.offset
            }
            .map(\.element)
        items = matches.map { .action($0) }
        status.stringValue = items.isEmpty ? "No matching commands." : "\(items.count) command\(items.count == 1 ? "" : "s")"
        table.reloadData()
        if !items.isEmpty { table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false); table.scrollRowToVisible(0) }
        chooseButton.isEnabled = !items.isEmpty
    }
    private func filterSymbols(query: String) {
        scope.stringValue = "Symbols · Jump to declaration in the current inspected file"
        chooseButton.title = "Jump to Symbol"
        let available = symbolsProvider()
        let terms = query.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: .current).split(whereSeparator: { $0.isWhitespace })
        let matches = available.filter { item in
            guard !terms.isEmpty else { return true }
            let target = "\(item.name) \(item.kind)".folding(options: [.caseInsensitive, .diacriticInsensitive], locale: .current)
            return terms.allSatisfy { target.contains($0) }
        }
        items = matches.map { .symbol(name: $0.name, kind: $0.kind, line: $0.line, index: $0.index) }
        status.stringValue = available.isEmpty ? "Open a source file in Inspect first to jump to its declarations." : items.isEmpty ? "No matching symbols on this page." : "\(items.count) symbol\(items.count == 1 ? "" : "s")"
        table.reloadData()
        if !items.isEmpty { table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false); table.scrollRowToVisible(0) }
        chooseButton.isEnabled = !items.isEmpty
    }
    private func filterCheckpoints(query: String) {
        scope.stringValue = "Checkpoints · Open or review conversation snapshots"
        chooseButton.title = "Open Checkpoint"
        let available = checkpointsProvider()
        let terms = query.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: .current).split(whereSeparator: { $0.isWhitespace })
        let matches = available.filter { item in
            guard !terms.isEmpty else { return true }
            let target = "\(item.title) \(item.detail) \(item.id)".folding(options: [.caseInsensitive, .diacriticInsensitive], locale: .current)
            return terms.allSatisfy { target.contains($0) }
        }
        items = matches.map { .checkpoint(id: $0.id, title: $0.title, detail: $0.detail) }
        status.stringValue = available.isEmpty ? "No recorded checkpoints in this conversation yet." : items.isEmpty ? "No matching checkpoints." : "\(items.count) checkpoint\(items.count == 1 ? "" : "s")"
        table.reloadData()
        if !items.isEmpty { table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false); table.scrollRowToVisible(0) }
        chooseButton.isEnabled = !items.isEmpty
    }
    private func filterLine(query: String) {
        scope.stringValue = "Go to Line · Type a line number (and optional :column) to jump"
        chooseButton.title = "Go to Line"
        let availableLines = linesProvider()
        let totalCount = max(1, availableLines.count)
        let parts = query.split(separator: ":").map(String.init)
        let targetLine = parts.first.flatMap(Int.init) ?? 1
        let clampedLine = max(1, min(targetLine, totalCount))
        let targetCol = parts.count > 1 ? (parts.last.flatMap(Int.init) ?? 1) : 1
        let clampedCol = max(1, targetCol)
        let snippet = availableLines.indices.contains(clampedLine - 1) ? availableLines[clampedLine - 1].trimmingCharacters(in: .whitespaces) : ""
        items = [.lineNumber(line: clampedLine, column: clampedCol, snippet: snippet)]
        status.stringValue = availableLines.isEmpty ? "Current inspected file is empty." : "Line \(clampedLine) of \(totalCount)\(clampedCol > 1 ? ", Column \(clampedCol)" : "")"
        table.reloadData()
        table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false); table.scrollRowToVisible(0)
        chooseButton.isEnabled = true
    }
    static func parseFileLineCol(_ input: String) -> (path: String, line: Int?, col: Int?) {
        // 1. GitHub / Markdown anchor syntax: path#L42, path#L42C10, or path#L42-L50
        if let hashRange = input.range(of: "#L", options: .caseInsensitive) {
            let basePath = String(input[..<hashRange.lowerBound])
            let anchor = String(input[hashRange.upperBound...])
            let anchorPrefix = anchor.split(separator: "-").first.map(String.init) ?? anchor
            let cParts = anchorPrefix.split(separator: "C", omittingEmptySubsequences: false).map(String.init)
            if let l = cParts.first.flatMap(Int.init), l > 0 {
                let c = (cParts.count >= 2 ? Int(cParts[1]) : nil).flatMap { $0 > 0 ? $0 : nil }
                return (basePath, l, c)
            }
        }
        // 2. Compiler / stacktrace parenthesis syntax: path(42) or path(42,10)
        if input.hasSuffix(")"), let openParen = input.lastIndex(of: "("), openParen > input.startIndex {
            let basePath = String(input[..<openParen])
            let inner = String(input[input.index(after: openParen)..<input.index(before: input.endIndex)])
            let pParts = inner.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }
            if let l = pParts.first.flatMap(Int.init), l > 0 {
                let c = (pParts.count >= 2 ? Int(pParts[1]) : nil).flatMap { $0 > 0 ? $0 : nil }
                return (basePath, l, c)
            }
        }
        // 3. Standard colon syntax: path:42 or path:42:10
        let parts = input.split(separator: ":", omittingEmptySubsequences: false).map(String.init)
        if parts.count >= 2, let l = Int(parts[1]), l > 0 {
            let c = (parts.count >= 3 ? Int(parts[2]) : nil).flatMap { $0 > 0 ? $0 : nil }
            return (parts[0], l, c)
        }
        return (input, nil, nil)
    }

    @objc private func refresh() { debounce?.cancel(); performSearch(refresh: true) }
    private func performSearch(refresh: Bool = false) {
        guard !dismissed else { return }
        let rawQuery = search.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
        guard rawQuery.utf16.count <= 200 else { clearResults("Use a shorter filename or path (up to 200 characters)."); return }
        clearResults("Searching filenames…")
        let (fileQuery, _, _) = QuickOpenWindow.parseFileLineCol(rawQuery)
        requests.submit(fileQuery, refresh: refresh)
    }
    private func clearResults(_ message: String) {
        items = []; table.deselectAll(nil); table.reloadData(); chooseButton.isEnabled = false
        status.stringValue = message
    }
    private func received(_ result: Result<[String: Any], Error>) {
        switch result {
        case .failure:
            clearResults("Could not search files. Choose Refresh to retry, or browse files in Code.")
            retry.title = "Retry"
        case .success(let data):
            let rawQuery = search.stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            let (_, lineJump, colJump) = QuickOpenWindow.parseFileLineCol(rawQuery)
            let suffix = lineJump.map { ":\($0)\(colJump.map { ":\($0)" } ?? "")" } ?? ""
            let fileEntries: [(path: String, name: String)] = (data["entries"] as? [[String: Any]] ?? []).prefix(50).compactMap { item in
                guard let path = item["path"] as? String, WalkthroughActivity.safeRelativePath(path) else { return nil }
                return (path + suffix, (path as NSString).lastPathComponent + suffix)
            }
            items = fileEntries.map { .file(path: $0.path, name: $0.name) }
            scope.stringValue = data["scope"] as? String == "git" ? "Project files · Git ignore rules apply. File contents aren’t searched." : "Workspace files · Hidden and generated folders are excluded."
            chooseButton.title = lineJump == nil ? "Open Preview" : "Go to Line in File"
            let limited = data["scanLimited"] as? Bool == true
            status.stringValue = items.isEmpty ? (limited ? "No matches in the scanned files. Browse Code to look further." : "No matching files. Try a filename or part of its path.") : "\(items.count) file\(items.count == 1 ? "" : "s")\(data["more"] as? Bool == true ? " shown · keep typing to narrow results" : "")\(limited ? " · partial workspace scan" : "")"
            retry.title = "Refresh"
            table.reloadData()
            if !items.isEmpty { table.selectRowIndexes(IndexSet(integer: 0), byExtendingSelection: false); table.scrollRowToVisible(0) }
            chooseButton.isEnabled = !items.isEmpty
        }
    }
    func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
        guard !textView.hasMarkedText() else { return false }
        if commandSelector == #selector(NSResponder.cancelOperation(_:)) { dismiss(); return true }
        if commandSelector == #selector(NSResponder.insertNewline(_:)) { choose(); return true }
        if [#selector(NSResponder.moveDown(_:)), #selector(NSResponder.moveUp(_:))].contains(commandSelector) {
            guard !items.isEmpty else { return true }
            let delta = commandSelector == #selector(NSResponder.moveDown(_:)) ? 1 : -1
            let row = max(0, min(items.count - 1, table.selectedRow + delta))
            table.selectRowIndexes(IndexSet(integer: row), byExtendingSelection: false); table.scrollRowToVisible(row); return true
        }
        return false
    }
    @objc private func choose() {
        guard items.indices.contains(table.selectedRow), !dismissed else { return }
        let selected = items[table.selectedRow]
        dismiss()
        switch selected {
        case .file(let path, _):
            openFile(path)
        case .action(let action):
            action.perform()
        case .symbol(_, _, _, let index):
            onSelectSymbol?(index)
        case .checkpoint(let id, _, _):
            onSelectCheckpoint?(id)
        case .lineNumber(let line, let col, _):
            onSelectLine?(line, col)
        }
    }
    func tableViewSelectionDidChange(_ notification: Notification) { chooseButton.isEnabled = items.indices.contains(table.selectedRow) }
    func numberOfRows(in tableView: NSTableView) -> Int { items.count }
    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard items.indices.contains(row) else { return nil }
        let item = items[row], cell = NSTableCellView()
        let name: NSTextField, path: NSTextField, icon: NSImageView
        switch item {
        case .file(let filePath, let fileName):
            name = JourneyStyle.label(fileName, size: 13, weight: .medium)
            path = JourneyStyle.label(filePath, size: 11, secondary: true)
            icon = NSImageView(image: NSImage(systemSymbolName: "doc", accessibilityDescription: nil)!)
            cell.toolTip = filePath
        case .action(let action):
            name = JourneyStyle.label(action.title, size: 13, weight: .medium)
            path = JourneyStyle.label(action.shortcut, size: 11, secondary: true)
            icon = NSImageView(image: NSImage(systemSymbolName: "command", accessibilityDescription: nil)!)
            cell.toolTip = action.title
        case .symbol(let symbolName, let kind, let line, _):
            name = JourneyStyle.label(symbolName, size: 13, weight: .medium)
            path = JourneyStyle.label("\(kind) · Line \(line)", size: 11, secondary: true)
            icon = NSImageView(image: NSImage(systemSymbolName: "number", accessibilityDescription: nil)!)
            cell.toolTip = "\(symbolName) (\(kind))"
        case .checkpoint(_, let title, let detail):
            name = JourneyStyle.label(title, size: 13, weight: .medium)
            path = JourneyStyle.label(detail, size: 11, secondary: true)
            icon = NSImageView(image: NSImage(systemSymbolName: "clock.arrow.circlepath", accessibilityDescription: nil)!)
            cell.toolTip = "\(title) · \(detail)"
        case .lineNumber(let line, let col, let snippet):
            name = JourneyStyle.label("Line \(line)\(col > 1 ? ", Column \(col)" : "")", size: 13, weight: .medium)
            path = JourneyStyle.label(snippet.isEmpty ? "Jump to line in current inspected file" : snippet, size: 11, secondary: true)
            icon = NSImageView(image: NSImage(systemSymbolName: "arrow.down.to.line", accessibilityDescription: nil) ?? NSImage(systemSymbolName: "doc.text", accessibilityDescription: nil)!)
            cell.toolTip = "Go to line \(line)"
        }
        for label in [name, path] { label.maximumNumberOfLines = 1; label.lineBreakMode = .byTruncatingMiddle }
        let labels = JourneyStyle.stack([name, path], spacing: 3)
        icon.widthAnchor.constraint(equalToConstant: 18).isActive = true
        let content = JourneyStyle.stack([icon, labels], vertical: false, spacing: 10)
        cell.addSubview(content); JourneyStyle.pin(content, to: cell, inset: 5)
        for label in [name, path] { label.widthAnchor.constraint(equalTo: labels.widthAnchor).isActive = true }
        cell.textField = name; cell.imageView = icon
        return cell
    }
    func searchProbe(_ query: String) {
        search.stringValue = query
        if query.hasPrefix(">") {
            filterActions(query: String(query.dropFirst()).trimmingCharacters(in: .whitespaces))
        } else if query.hasPrefix("@") {
            filterSymbols(query: String(query.dropFirst()).trimmingCharacters(in: .whitespaces))
        } else if query.hasPrefix("#") {
            filterCheckpoints(query: String(query.dropFirst()).trimmingCharacters(in: .whitespaces))
        } else if query.hasPrefix(":") {
            filterLine(query: String(query.dropFirst()).trimmingCharacters(in: .whitespaces))
        } else {
            performSearch()
        }
    }
    func openSelectedProbe() { choose() }
    var diagnostics: [String: Any] {
        let filePaths = items.compactMap { if case .file(let path, _) = $0 { return path }; return nil }
        return ["visible": window?.isVisible == true, "count": items.count, "query": search.stringValue, "status": status.stringValue, "scope": scope.stringValue, "paths": filePaths]
    }
    deinit { debounce?.cancel(); requests.close() }
}
