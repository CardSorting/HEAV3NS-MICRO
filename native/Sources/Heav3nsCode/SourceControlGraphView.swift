// [LAYER: PLUMBING]
import AppKit

struct GitGraphCommit {
    let hash: String
    let shortHash: String
    let subject: String
    let author: String
    let relativeTime: String
    let refs: [String]
    let isHead: Bool
}

private final class CommitGraphRailView: NSView {
    var isHead = false
    var isFirst = false
    var isLast = false
    override var isFlipped: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        let midX = bounds.midX
        let midY = bounds.midY
        NSColor.systemBlue.withAlphaComponent(0.75).setStroke()
        let line = NSBezierPath()
        line.lineWidth = 1.75
        let topY: CGFloat = isFirst ? midY : 0
        let botY: CGFloat = isLast ? midY : bounds.height
        if botY > topY {
            line.move(to: NSPoint(x: midX, y: topY))
            line.line(to: NSPoint(x: midX, y: botY))
            line.stroke()
        }
        let dotRect = NSRect(x: midX - 4, y: midY - 4, width: 8, height: 8)
        let dot = NSBezierPath(ovalIn: dotRect)
        if isHead {
            NSColor.controlBackgroundColor.setFill()
            dot.fill()
            NSColor.systemBlue.setStroke()
            dot.lineWidth = 1.75
            dot.stroke()
        } else {
            NSColor.systemBlue.setFill()
            dot.fill()
        }
    }
}

/// VS Code-style bottom Source Control Graph pane showing commit rail, branch pills, and quick sync actions.
final class SourceControlGraphView: NSView, NSTableViewDataSource, NSTableViewDelegate {
    var onSelectCommit: ((String) -> Void)?
    var onFetch: (() -> Void)?
    var onPull: (() -> Void)?
    var onPush: (() -> Void)?
    var onRefresh: (() -> Void)?
    var onCopyHash: ((String) -> Void)?

    private var commits = [GitGraphCommit]()
    private let titleLabel = NSTextField(labelWithString: "Graph")
    private let repoLabel = NSTextField(labelWithString: "")
    private let table = NSTableView()
    private let scroll = NSScrollView()
    private var fetchBtn: JourneyButton!
    private var pullBtn: JourneyButton!
    private var pushBtn: JourneyButton!
    private var refreshBtn: JourneyButton!

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        translatesAutoresizingMaskIntoConstraints = false
        let topRule = NSBox()
        topRule.boxType = .separator
        topRule.translatesAutoresizingMaskIntoConstraints = false

        titleLabel.font = .systemFont(ofSize: 11, weight: .semibold)
        repoLabel.font = .systemFont(ofSize: 10.5)
        repoLabel.textColor = .secondaryLabelColor
        repoLabel.lineBreakMode = .byTruncatingMiddle
        repoLabel.setContentHuggingPriority(.init(1), for: .horizontal)

        fetchBtn = JourneyButton("", symbol: "arrow.down.to.line") { [weak self] in self?.onFetch?() }
        fetchBtn.bezelStyle = .inline; fetchBtn.isBordered = false; fetchBtn.toolTip = "Fetch from remote"
        pullBtn = JourneyButton("", symbol: "arrow.down.circle") { [weak self] in self?.onPull?() }
        pullBtn.bezelStyle = .inline; pullBtn.isBordered = false; pullBtn.toolTip = "Pull (rebase)"
        pushBtn = JourneyButton("", symbol: "arrow.up.circle") { [weak self] in self?.onPush?() }
        pushBtn.bezelStyle = .inline; pushBtn.isBordered = false; pushBtn.toolTip = "Push to remote"
        refreshBtn = JourneyButton("", symbol: "arrow.clockwise") { [weak self] in self?.onRefresh?() }
        refreshBtn.bezelStyle = .inline; refreshBtn.isBordered = false; refreshBtn.toolTip = "Refresh commit graph"

        let header = NSStackView(views: [titleLabel, repoLabel, fetchBtn, pullBtn, pushBtn, refreshBtn])
        header.orientation = .horizontal; header.alignment = .centerY; header.spacing = 4
        header.translatesAutoresizingMaskIntoConstraints = false

        table.addTableColumn(NSTableColumn(identifier: NSUserInterfaceItemIdentifier("commit")))
        table.headerView = nil; table.dataSource = self; table.delegate = self
        table.style = .plain; table.backgroundColor = .clear; table.rowHeight = 24
        table.columnAutoresizingStyle = .uniformColumnAutoresizingStyle
        table.target = self; table.action = #selector(rowClicked)

        scroll.documentView = table; scroll.hasVerticalScroller = true; scroll.drawsBackground = false
        scroll.translatesAutoresizingMaskIntoConstraints = false

        [topRule, header, scroll].forEach(addSubview)
        NSLayoutConstraint.activate([
            topRule.topAnchor.constraint(equalTo: topAnchor),
            topRule.leadingAnchor.constraint(equalTo: leadingAnchor),
            topRule.trailingAnchor.constraint(equalTo: trailingAnchor),
            header.topAnchor.constraint(equalTo: topRule.bottomAnchor, constant: 5),
            header.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 6),
            header.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -6),
            scroll.topAnchor.constraint(equalTo: header.bottomAnchor, constant: 4),
            scroll.leadingAnchor.constraint(equalTo: leadingAnchor),
            scroll.trailingAnchor.constraint(equalTo: trailingAnchor),
            scroll.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    func update(commits: [GitGraphCommit], repoName: String, hasRemote: Bool) {
        self.commits = commits
        repoLabel.stringValue = repoName
        fetchBtn.isEnabled = hasRemote
        pullBtn.isEnabled = hasRemote
        pushBtn.isEnabled = hasRemote
        table.reloadData()
    }

    @objc private func rowClicked() {
        let row = table.selectedRow
        guard commits.indices.contains(row) else { return }
        onSelectCommit?(commits[row].hash)
    }

    func numberOfRows(in tableView: NSTableView) -> Int { commits.count }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        guard commits.indices.contains(row) else { return nil }
        let commit = commits[row]
        let cell = NSTableCellView()
        let rail = CommitGraphRailView()
        rail.isHead = commit.isHead
        rail.isFirst = row == 0
        rail.isLast = row == commits.count - 1
        rail.widthAnchor.constraint(equalToConstant: 14).isActive = true
        rail.heightAnchor.constraint(equalToConstant: 24).isActive = true

        let subject = NSTextField(labelWithString: commit.subject)
        subject.font = .systemFont(ofSize: 11.5, weight: commit.isHead ? .semibold : .regular)
        subject.lineBreakMode = .byTruncatingTail
        subject.setContentHuggingPriority(.init(1), for: .horizontal)
        subject.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        var rowViews: [NSView] = [rail, subject]
        if let primaryRef = commit.refs.first, !primaryRef.isEmpty {
            let pill = NSTextField(labelWithString: " \(primaryRef) ")
            pill.font = .systemFont(ofSize: 9.5, weight: .semibold)
            pill.textColor = .systemBlue
            pill.wantsLayer = true
            pill.layer?.cornerRadius = 4
            pill.layer?.borderWidth = 1
            pill.layer?.borderColor = NSColor.systemBlue.withAlphaComponent(0.6).cgColor
            pill.layer?.backgroundColor = NSColor.systemBlue.withAlphaComponent(0.12).cgColor
            pill.setContentHuggingPriority(.required, for: .horizontal)
            rowViews.append(pill)
        }
        let stack = NSStackView(views: rowViews)
        stack.orientation = .horizontal; stack.alignment = .centerY; stack.spacing = 5
        stack.translatesAutoresizingMaskIntoConstraints = false
        cell.addSubview(stack)
        cell.toolTip = "\(commit.shortHash) · \(commit.subject)\n\(commit.author) · \(commit.relativeTime)"
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 4),
            stack.trailingAnchor.constraint(equalTo: cell.trailingAnchor, constant: -6),
            stack.centerYAnchor.constraint(equalTo: cell.centerYAnchor)
        ])
        return cell
    }

    func dispose() {
        onSelectCommit = nil; onFetch = nil; onPull = nil; onPush = nil; onRefresh = nil; onCopyHash = nil
    }
}
