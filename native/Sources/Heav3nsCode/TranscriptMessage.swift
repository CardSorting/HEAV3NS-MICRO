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

    init(_ row: TranscriptRow, availableWidth: CGFloat) {
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
            let bounded = CodeOutline.boundedDiffPreview(diffText, maxLines: 18).preview
            let innerWidth = max(40, textWidth - 16)
            let attr = CodeOutline.highlight(bounded, diff: true, fontSize: 11)
            let diffHeight = ceil(attr.boundingRect(with: NSSize(width: innerWidth, height: .greatestFiniteMagnitude),
                options: [.usesLineFragmentOrigin, .usesFontLeading]).height) + 4
            let box = NSRect(x: 12, y: y + 6, width: textWidth, height: diffHeight + 12)
            diffBox = box
            diffField = NSRect(x: 8, y: 6, width: innerWidth, height: diffHeight)
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

private final class TranscriptDiffSurface: NSView {
    override var isFlipped: Bool { true }
    override func draw(_ dirtyRect: NSRect) {
        NSColor.textBackgroundColor.withAlphaComponent(0.75).setFill()
        let path = NSBezierPath(roundedRect: bounds, xRadius: 8, yRadius: 8)
        path.fill()
        NSColor.separatorColor.withAlphaComponent(0.55).setStroke()
        path.lineWidth = 1
        path.stroke()
    }
}

final class TranscriptMessageCell: NSTableCellView {
    override var isFlipped: Bool { true }
    let row: TranscriptRow
    private let surface = TranscriptMessageSurface()
    private let title = NSTextField(wrappingLabelWithString: "")
    private let body = NSTextField(wrappingLabelWithString: "")
    private let evidence = NSTextField(wrappingLabelWithString: "")
    private let diffBox = TranscriptDiffSurface()
    private let diffField = NSTextField(wrappingLabelWithString: "")
    private let icon = NSImageView()
    private var disclosure: JourneyButton?
    private var openDiffButton: JourneyButton?
    private var copyDiffButton: JourneyButton?
    private var checkpoint: JourneyButton?
    private var actionButton: JourneyButton?

    init(_ row: TranscriptRow, expanded: Bool, toggle: @escaping () -> Void, openCheckpoint: @escaping () -> Void, openAction: (() -> Void)? = nil, openDiff: (() -> Void)? = nil) {
        self.row = row
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
            let bounded = CodeOutline.boundedDiffPreview(diffText, maxLines: 18).preview
            diffField.attributedStringValue = CodeOutline.highlight(bounded, diff: true, fontSize: 11)
            diffField.isSelectable = true
            diffBox.addSubview(diffField)
            surface.addSubview(diffBox)
            let copyBtn = JourneyButton("Copy Diff", symbol: "doc.on.doc") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(diffText, forType: .string)
            }
            copyBtn.bezelStyle = .inline; copyBtn.controlSize = .small
            copyBtn.toolTip = "Copy full unified diff to clipboard"
            copyDiffButton = copyBtn; surface.addSubview(copyBtn)
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
    override func layout() {
        super.layout()
        let geometry = TranscriptMessageLayout(row, availableWidth: bounds.width)
        surface.frame = NSRect(x: row.user ? bounds.width - 12 - geometry.width : 12, y: 2, width: geometry.width, height: geometry.height - 4)
        title.frame = geometry.title
        body.frame = geometry.body ?? .zero
        evidence.frame = geometry.evidence ?? .zero
        for field in [title, body, evidence] where !field.isHidden { field.preferredMaxLayoutWidth = field.frame.width }
        if let boxFrame = geometry.diffBox, let fieldFrame = geometry.diffField, !diffBox.isHidden {
            diffBox.frame = boxFrame
            diffField.frame = fieldFrame
            diffField.preferredMaxLayoutWidth = fieldFrame.width
        }
        icon.frame = NSRect(x: 12, y: 14, width: 16, height: 16)
        disclosure?.frame = NSRect(x: 10, y: 12, width: 20, height: 20)
        if var frame = geometry.openDiff, let openDiffButton {
            frame.size.width = min(frame.width, openDiffButton.intrinsicContentSize.width)
            openDiffButton.frame = frame
            if var copyFrame = geometry.copyDiff, let copyDiffButton {
                let x = frame.maxX + 8
                copyFrame.origin.x = x
                copyFrame.size.width = min(max(0, geometry.width - 12 - x), copyDiffButton.intrinsicContentSize.width)
                copyDiffButton.frame = copyFrame
            }
        } else if var copyFrame = geometry.copyDiff, let copyDiffButton {
            copyFrame.size.width = min(copyFrame.width, copyDiffButton.intrinsicContentSize.width)
            copyDiffButton.frame = copyFrame
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
        [disclosure, openDiffButton, copyDiffButton, checkpoint, actionButton].forEach { $0?.removeFromSuperview() }
        disclosure = nil; openDiffButton = nil; copyDiffButton = nil; checkpoint = nil; actionButton = nil
    }
}
