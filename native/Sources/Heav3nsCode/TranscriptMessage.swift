// [LAYER: PLUMBING]
import AppKit

struct TranscriptRow: Equatable {
    let id: String
    let title: String
    let text: String
    let activity: Bool
    var user = false
    var checkpoint: String?
    var evidence: String?
    var automatic = false
    var runID: String?
    var activityID: String?
    var actionTitle: String?
    var diffText: String? = nil
    var diffFile: String? = nil
    var diffStats: String? = nil
    var diffLive: Bool = false
}

/// Row height and rendered text share the actual cell width, including AppKit's
/// table-style insets. Explicit native text frames keep wrapping and measurement
/// together without reloading the conversation or disturbing selection on resize.
struct TranscriptMessageLayout {
    let width: CGFloat
    let height: CGFloat
    let title: NSRect
    let evidence: NSRect?
    let body: NSRect?
    let diffBox: NSRect?
    let diffField: NSRect?
    let openDiff: NSRect?
    let copyDiff: NSRect?
    let checkpoint: NSRect?
    let action: NSRect?

    init(_ row: TranscriptRow, availableWidth: CGFloat, expanded: Bool = false) {
        width = max(120, min(760, availableWidth - 24, row.user ? availableWidth * 0.86 : availableWidth))
        let textWidth = width - 24
        func measured(_ text: String, width: CGFloat, font: NSFont) -> CGFloat {
            ceil((text as NSString).boundingRect(with: NSSize(width: max(1, width), height: .greatestFiniteMagnitude),
                options: [.usesLineFragmentOrigin, .usesFontLeading], attributes: [.font: font]).height) + 2
        }
        let titleHeight = max(20, measured(row.title, width: textWidth - 22, font: .systemFont(ofSize: 12, weight: .semibold)))
        title = NSRect(x: 34, y: 12, width: textWidth - 22, height: titleHeight)
        var y = title.maxY
        if let text = row.evidence {
            let frame = NSRect(x: 12, y: y + 5, width: textWidth, height: measured(text, width: textWidth, font: .systemFont(ofSize: 12)))
            evidence = frame; y = frame.maxY
        } else { evidence = nil }
        if !row.text.isEmpty {
            let frame = NSRect(x: 12, y: y + 5, width: textWidth, height: measured(row.text, width: textWidth, font: .systemFont(ofSize: row.activity ? 12 : 14)))
            body = frame; y = frame.maxY
        } else { body = nil }
        if let diffText = row.diffText, !diffText.isEmpty {
            let displayed = expanded ? diffText : CodeOutline.compressedDiffPreview(diffText, maxChangedLines: 8).preview
            let innerWidth = max(40, textWidth - 20)
            let attr = CodeOutline.highlight(displayed, diff: true, fontSize: 11)
            let diffHeight = ceil(attr.boundingRect(with: NSSize(width: innerWidth, height: .greatestFiniteMagnitude),
                options: [.usesLineFragmentOrigin, .usesFontLeading]).height) + 4
            let headerHeight: CGFloat = 20
            let box = NSRect(x: 12, y: y + 6, width: textWidth, height: headerHeight + diffHeight + 14)
            diffBox = box
            diffField = NSRect(x: 12, y: 6 + headerHeight, width: innerWidth, height: diffHeight)
            y = box.maxY
        } else {
            diffBox = nil
            diffField = nil
        }
        if row.diffFile != nil || row.diffText != nil {
            let frame = NSRect(x: 12, y: y + 5, width: textWidth, height: 24)
            openDiff = row.diffFile != nil ? frame : nil
            copyDiff = row.diffText != nil ? frame : nil
            y = frame.maxY
        } else {
            openDiff = nil
            copyDiff = nil
        }
        if row.actionTitle != nil {
            let frame = NSRect(x: 12, y: y + 5, width: textWidth, height: 24)
            action = frame; y = frame.maxY
        } else { action = nil }
        if row.checkpoint != nil {
            let frame = NSRect(x: 12, y: y + 5, width: textWidth, height: 24)
            checkpoint = frame; y = frame.maxY
        } else { checkpoint = nil }
        height = ceil(y + 16) // 12pt bottom inset and 2pt outside each end of the surface.
    }
}

private final class TranscriptMessageSurface: NSView {
    var user = false
    var activity = false
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        let color: NSColor = user ? .controlAccentColor : activity ? .systemTeal : .labelColor
        color.withAlphaComponent(user ? 0.10 : activity ? 0.07 : 0.035).setFill()
        NSBezierPath(roundedRect: bounds, xRadius: 12, yRadius: 12).fill()
    }
}

private enum StreamingDiffRevealTracker {
    @MainActor static var revealedCharsByRowID = [String: Int]()

    @MainActor static func record(_ rowID: String, count: Int) {
        if revealedCharsByRowID.count > 200 && revealedCharsByRowID[rowID] == nil {
            revealedCharsByRowID.removeAll(keepingCapacity: true)
        }
        revealedCharsByRowID[rowID] = count
    }

    static func nextTokenBoundary(in text: String, from index: String.Index, remaining: Int) -> String.Index {
        guard index < text.endIndex else { return text.endIndex }
        let maxStep = max(4, min(14, remaining / 40))
        var idx = index
        var step = 0
        while idx < text.endIndex && step < maxStep {
            let ch = text[idx]
            idx = text.index(after: idx)
            step += 1
            if step >= 2 && (ch == " " || ch == "\n" || ch == "." || ch == "(" || ch == ")" || ch == "," || ch == ";") {
                break
            }
        }
        return idx
    }
}

private final class TranscriptDiffSurface: NSView {
    override var isFlipped: Bool { true }
    var isLive = false { didSet { needsDisplay = true } }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.textBackgroundColor.withAlphaComponent(isLive ? 0.86 : 0.75).setFill()
        let path = NSBezierPath(roundedRect: bounds, xRadius: 8, yRadius: 8)
        path.fill()
        let borderColor = isLive ? NSColor.systemTeal.withAlphaComponent(0.60) : NSColor.separatorColor.withAlphaComponent(0.55)
        borderColor.setStroke()
        path.lineWidth = isLive ? 1.25 : 1
        path.stroke()

        let accentRect = NSRect(x: 0, y: 0, width: 3.5, height: bounds.height)
        let accentPath = NSBezierPath(roundedRect: accentRect, xRadius: 2, yRadius: 2)
        (isLive ? NSColor.systemTeal : NSColor.systemBlue.withAlphaComponent(0.55)).setFill()
        accentPath.fill()
    }

    func dispose() {}
}

final class TranscriptMessageCell: NSTableCellView {
    override var isFlipped: Bool { true }
    let row: TranscriptRow
    private let surface = TranscriptMessageSurface()
    private let title = NSTextField(wrappingLabelWithString: "")
    private let body = NSTextField(wrappingLabelWithString: "")
    private let evidence = NSTextField(wrappingLabelWithString: "")
    private let diffBox = TranscriptDiffSurface()
    private let diffHeader = NSTextField(labelWithString: "")
    private let diffLiveDot = NSView()
    private let diffField = NSTextField(wrappingLabelWithString: "")
    private var diffRevealTimer: Timer?
    private let icon = NSImageView()
    private let expanded: Bool
    private var toggleAction: (() -> Void)?
    private var diffClickRecognizer: NSClickGestureRecognizer?
    private var disclosure: JourneyButton?
    private var openDiffButton: JourneyButton?
    private var copyDiffButton: JourneyButton?
    private var expandDiffButton: JourneyButton?
    private var checkpoint: JourneyButton?
    private var actionButton: JourneyButton?

    init(_ row: TranscriptRow, expanded: Bool, toggle: @escaping () -> Void, openCheckpoint: @escaping () -> Void, openAction: (() -> Void)? = nil, openDiff: (() -> Void)? = nil) {
        self.row = row
        self.expanded = expanded
        self.toggleAction = toggle
        super.init(frame: .zero)
        surface.user = row.user; surface.activity = row.activity; addSubview(surface)
        title.stringValue = row.title; title.font = .systemFont(ofSize: 12, weight: .semibold)
        title.textColor = row.activity ? .secondaryLabelColor : .labelColor
        body.stringValue = row.text; body.font = .systemFont(ofSize: row.activity ? 12 : 14)
        body.textColor = row.activity ? .secondaryLabelColor : .labelColor; body.isSelectable = true
        evidence.stringValue = row.evidence ?? ""; evidence.font = .systemFont(ofSize: 12); evidence.textColor = .secondaryLabelColor
        [title, body, evidence].forEach { surface.addSubview($0) }
        body.isHidden = row.text.isEmpty; evidence.isHidden = row.evidence == nil

        if let diffText = row.diffText, !diffText.isEmpty {
            let compressed = CodeOutline.compressedDiffPreview(diffText, maxChangedLines: 8)
            let displayed = expanded ? diffText : compressed.preview
            diffBox.isLive = row.diffLive
            diffLiveDot.wantsLayer = true
            diffLiveDot.layer?.cornerRadius = 3.5
            diffLiveDot.layer?.backgroundColor = NSColor.systemGreen.cgColor
            diffLiveDot.isHidden = !row.diffLive
            if row.diffLive {
                let pulse = CABasicAnimation(keyPath: "opacity")
                pulse.fromValue = 1.0
                pulse.toValue = 0.25
                pulse.duration = 0.85
                pulse.autoreverses = true
                pulse.repeatCount = .infinity
                pulse.isRemovedOnCompletion = false
                diffLiveDot.layer?.add(pulse, forKey: "livePulse")
            }
            let headerAttr = NSMutableAttributedString(
                string: row.diffLive ? "LIVE STREAMING DIFF" : (expanded ? "FULL DIFF" : "BRIEF DIFF"),
                attributes: [
                    .font: NSFont.systemFont(ofSize: 10, weight: .semibold),
                    .foregroundColor: row.diffLive ? NSColor.systemTeal : NSColor.secondaryLabelColor
                ]
            )
            if let stats = row.diffStats, !stats.isEmpty {
                headerAttr.append(NSAttributedString(string: " · ", attributes: [.font: NSFont.systemFont(ofSize: 10, weight: .medium), .foregroundColor: NSColor.tertiaryLabelColor]))
                let parts = stats.trimmingCharacters(in: CharacterSet(charactersIn: "()")).components(separatedBy: "/")
                if parts.count == 2 {
                    headerAttr.append(NSAttributedString(string: parts[0].trimmingCharacters(in: .whitespaces), attributes: [.font: NSFont.monospacedSystemFont(ofSize: 10, weight: .semibold), .foregroundColor: NSColor.systemGreen]))
                    headerAttr.append(NSAttributedString(string: " ", attributes: [.font: NSFont.systemFont(ofSize: 10)]))
                    headerAttr.append(NSAttributedString(string: parts[1].trimmingCharacters(in: .whitespaces), attributes: [.font: NSFont.monospacedSystemFont(ofSize: 10, weight: .semibold), .foregroundColor: NSColor.systemRed]))
                } else {
                    headerAttr.append(NSAttributedString(string: stats, attributes: [.font: NSFont.monospacedSystemFont(ofSize: 10, weight: .semibold), .foregroundColor: NSColor.secondaryLabelColor]))
                }
            }
            if !expanded && compressed.omittedLines > 0 {
                headerAttr.append(NSAttributedString(string: " · Click to expand", attributes: [.font: NSFont.systemFont(ofSize: 10, weight: .regular), .foregroundColor: NSColor.tertiaryLabelColor]))
            }
            diffHeader.attributedStringValue = headerAttr
            diffBox.addSubview(diffLiveDot)
            diffBox.addSubview(diffHeader)

            diffField.wantsLayer = true
            diffField.isSelectable = true
            diffBox.addSubview(diffField)

            let totalChars = displayed.count
            let prevRevealed = StreamingDiffRevealTracker.revealedCharsByRowID[row.id] ?? (row.diffLive ? 0 : totalChars)
            let startCount = max(0, min(prevRevealed, totalChars))
            if row.diffLive && startCount < totalChars {
                var currentOffset = startCount
                var currentIndex = displayed.index(displayed.startIndex, offsetBy: startCount)
                currentIndex = StreamingDiffRevealTracker.nextTokenBoundary(in: displayed, from: currentIndex, remaining: totalChars - currentOffset)
                currentOffset = displayed.distance(from: displayed.startIndex, to: currentIndex)
                let initialSlice = String(displayed[..<currentIndex])
                diffField.attributedStringValue = CodeOutline.streamingDiffAttributed(initialSlice, hasMore: currentOffset < totalChars, fontSize: 11)
                StreamingDiffRevealTracker.record(row.id, count: currentOffset)
                if currentOffset < totalChars {
                    diffRevealTimer = Timer.scheduledTimer(withTimeInterval: 0.02, repeats: true) { [weak self] timer in
                        guard let self else { timer.invalidate(); return }
                        currentIndex = StreamingDiffRevealTracker.nextTokenBoundary(in: displayed, from: currentIndex, remaining: totalChars - currentOffset)
                        currentOffset = displayed.distance(from: displayed.startIndex, to: currentIndex)
                        let hasMore = currentOffset < totalChars
                        let slice = String(displayed[..<currentIndex])
                        MainActor.assumeIsolated {
                            self.diffField.attributedStringValue = CodeOutline.streamingDiffAttributed(slice, hasMore: hasMore, fontSize: 11)
                            StreamingDiffRevealTracker.record(row.id, count: currentOffset)
                        }
                        if !hasMore {
                            timer.invalidate()
                            self.diffRevealTimer = nil
                        }
                    }
                }
            } else {
                diffField.attributedStringValue = CodeOutline.highlight(displayed, diff: true, fontSize: 11)
                StreamingDiffRevealTracker.record(row.id, count: totalChars)
            }

            diffBox.toolTip = expanded ? "Click to compress diff" : "Click to expand full diff"
            let click = NSClickGestureRecognizer(target: self, action: #selector(handleDiffBoxClick))
            diffBox.addGestureRecognizer(click)
            diffClickRecognizer = click
            surface.addSubview(diffBox)
            let copyBtn = JourneyButton("Copy Diff", symbol: "doc.on.doc") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(diffText, forType: .string)
            }
            copyBtn.bezelStyle = .inline; copyBtn.controlSize = .small
            copyBtn.toolTip = "Copy full unified diff to clipboard"
            copyDiffButton = copyBtn; surface.addSubview(copyBtn)
            if expanded || compressed.omittedLines > 0 {
                let expandTitle = expanded ? "Compress" : "Expand Full Diff"
                let expandSymbol = expanded ? "arrow.down.right.and.arrow.up.left" : "arrow.up.left.and.arrow.down.right"
                let expandBtn = JourneyButton(expandTitle, symbol: expandSymbol, action: toggle)
                expandBtn.bezelStyle = .inline; expandBtn.controlSize = .small
                expandBtn.toolTip = expanded ? "Compress to brief changed lines" : "Expand to full unified diff"
                expandDiffButton = expandBtn; surface.addSubview(expandBtn)
            }
        } else {
            diffBox.isHidden = true
        }

        if row.activity {
            let button = JourneyButton("", symbol: expanded ? "chevron.down" : "chevron.right", action: toggle)
            button.isBordered = false; button.bezelStyle = .inline; button.imagePosition = .imageOnly
            button.setAccessibilityLabel("\(expanded ? "Collapse" : "Expand") \(row.title)")
            disclosure = button; surface.addSubview(button)
        } else {
            icon.image = NSImage(systemSymbolName: row.user ? "person.crop.circle.fill" : "sparkles", accessibilityDescription: nil)
            icon.contentTintColor = row.user ? .controlAccentColor : .secondaryLabelColor; surface.addSubview(icon)
        }
        if let file = row.diffFile, let openDiff {
            let button = JourneyButton("View Diff", symbol: "arrow.left.arrow.right.square", action: openDiff)
            button.bezelStyle = .inline; button.controlSize = .small
            button.toolTip = "Inspect highlighted working-tree diff for \(file)"
            openDiffButton = button; surface.addSubview(button)
        }
        if let actionTitle = row.actionTitle, let openAction {
            let button = JourneyButton(actionTitle, symbol: "list.bullet.rectangle", action: openAction)
            button.bezelStyle = .inline; button.controlSize = .small
            button.toolTip = "Open Walkthrough focused on this activity (⌘6)"
            actionButton = button; surface.addSubview(button)
        }
        if row.checkpoint != nil {
            let button = JourneyButton(row.automatic ? "Checkpoint before repair" : row.user ? "Checkpoint before message" : "Checkpoint after reply", symbol: "clock.arrow.circlepath", action: openCheckpoint)
            button.bezelStyle = .inline; button.controlSize = .small; button.toolTip = "Review and restore project files at this point"
            checkpoint = button; surface.addSubview(button)
        }
        setAccessibilityLabel(row.automatic ? "Automatic app repair" : row.user ? "Your message" : row.diffText != nil ? "Live highlighted diff" : row.activity ? "LUMI activity" : "LUMI response")
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }
    override func setFrameSize(_ newSize: NSSize) { super.setFrameSize(newSize); needsLayout = true }
    @objc private func handleDiffBoxClick() { toggleAction?() }
    override func layout() {
        super.layout()
        let geometry = TranscriptMessageLayout(row, availableWidth: bounds.width, expanded: expanded)
        surface.frame = NSRect(x: row.user ? bounds.width - 12 - geometry.width : 12, y: 2, width: geometry.width, height: geometry.height - 4)
        title.frame = geometry.title
        body.frame = geometry.body ?? .zero
        evidence.frame = geometry.evidence ?? .zero
        for field in [title, body, evidence] where !field.isHidden { field.preferredMaxLayoutWidth = field.frame.width }
        if let boxFrame = geometry.diffBox, let fieldFrame = geometry.diffField, !diffBox.isHidden {
            diffBox.frame = boxFrame
            if row.diffLive {
                diffLiveDot.frame = NSRect(x: 12, y: 9, width: 7, height: 7)
                diffHeader.frame = NSRect(x: 24, y: 5, width: max(40, boxFrame.width - 34), height: 15)
            } else {
                diffHeader.frame = NSRect(x: 12, y: 5, width: max(40, boxFrame.width - 22), height: 15)
            }
            diffField.frame = fieldFrame
            diffField.preferredMaxLayoutWidth = fieldFrame.width
        }
        icon.frame = NSRect(x: 12, y: 14, width: 16, height: 16)
        disclosure?.frame = NSRect(x: 10, y: 12, width: 20, height: 20)
        if var frame = geometry.openDiff, let openDiffButton {
            frame.size.width = min(frame.width, openDiffButton.intrinsicContentSize.width)
            openDiffButton.frame = frame
            var nextX = frame.maxX + 8
            if var copyFrame = geometry.copyDiff, let copyDiffButton {
                copyFrame.origin.x = nextX
                copyFrame.size.width = min(max(0, geometry.width - 12 - nextX), copyDiffButton.intrinsicContentSize.width)
                copyDiffButton.frame = copyFrame
                nextX = copyFrame.maxX + 8
            }
            if var expandFrame = geometry.copyDiff, let expandDiffButton {
                expandFrame.origin.x = nextX
                expandFrame.size.width = min(max(0, geometry.width - 12 - nextX), expandDiffButton.intrinsicContentSize.width)
                expandDiffButton.frame = expandFrame
            }
        } else if var copyFrame = geometry.copyDiff, let copyDiffButton {
            copyFrame.size.width = min(copyFrame.width, copyDiffButton.intrinsicContentSize.width)
            copyDiffButton.frame = copyFrame
            if var expandFrame = geometry.copyDiff, let expandDiffButton {
                let nextX = copyFrame.maxX + 8
                expandFrame.origin.x = nextX
                expandFrame.size.width = min(max(0, geometry.width - 12 - nextX), expandDiffButton.intrinsicContentSize.width)
                expandDiffButton.frame = expandFrame
            }
        }
        if var frame = geometry.action, let actionButton {
            frame.size.width = min(frame.width, actionButton.intrinsicContentSize.width)
            actionButton.frame = frame
        }
        if var frame = geometry.checkpoint, let checkpoint {
            frame.size.width = min(frame.width, checkpoint.intrinsicContentSize.width)
            checkpoint.frame = frame
        }
    }
    var contentFits: Bool {
        let fields = [title, body, evidence].filter { !$0.isHidden }
        let diffOk = diffBox.isHidden || (surface.bounds.contains(diffBox.frame) && diffBox.bounds.contains(diffField.frame))
        return bounds.insetBy(dx: -1, dy: -1).contains(surface.frame) && fields.allSatisfy { surface.bounds.contains($0.frame) } && diffOk
    }
    var layoutIssue: [String: Any]? {
        guard !contentFits else { return nil }
        return ["row": row.id, "cell": NSStringFromRect(bounds), "surface": NSStringFromRect(surface.frame),
            "labels": [title, body, evidence].filter { !$0.isHidden }.map { ["frame": NSStringFromRect($0.frame), "text": String($0.stringValue.prefix(80))] }]
    }
    func dispose() {
        diffRevealTimer?.invalidate(); diffRevealTimer = nil
        diffLiveDot.layer?.removeAllAnimations()
        diffBox.dispose()
        if let diffClickRecognizer { diffBox.removeGestureRecognizer(diffClickRecognizer) }
        diffClickRecognizer = nil; toggleAction = nil
        [disclosure, openDiffButton, copyDiffButton, expandDiffButton, checkpoint, actionButton].forEach { $0?.removeFromSuperview() }
        disclosure = nil; openDiffButton = nil; copyDiffButton = nil; expandDiffButton = nil; checkpoint = nil; actionButton = nil
    }
}
