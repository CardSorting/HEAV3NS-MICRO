// [LAYER: UI]
import AppKit
import Foundation

final class AttachmentTokenBar: NSStackView {
    private(set) var urls = [URL]()
    var onRemove: ((URL) -> Void)?
    var onChange: (() -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        orientation = .horizontal; alignment = .centerY; spacing = 6
        translatesAutoresizingMaskIntoConstraints = false
        isHidden = true
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    func add(url: URL) {
        guard !urls.contains(url) else { return }
        urls.append(url)
        render(); onChange?()
    }

    func remove(url: URL) {
        urls.removeAll { $0 == url }
        render(); onRemove?(url); onChange?()
    }

    func clear() {
        urls.removeAll(); render(); onChange?()
    }

    private func render() {
        arrangedSubviews.forEach { $0.removeFromSuperview() }
        isHidden = urls.isEmpty
        for url in urls {
            let pill = NSStackView()
            pill.orientation = .horizontal; pill.alignment = .centerY; pill.spacing = 4
            pill.edgeInsets = NSEdgeInsets(top: 2, left: 6, bottom: 2, right: 6)
            pill.wantsLayer = true; pill.layer?.cornerRadius = 5
            pill.layer?.backgroundColor = NSColor.quaternaryLabelColor.cgColor

            let icon = NSImageView(image: NSWorkspace.shared.icon(forFile: url.path))
            icon.widthAnchor.constraint(equalToConstant: 14).isActive = true
            icon.heightAnchor.constraint(equalToConstant: 14).isActive = true

            let name = JourneyStyle.label(url.lastPathComponent, size: 11, weight: .medium)
            name.maximumNumberOfLines = 1; name.lineBreakMode = .byTruncatingMiddle

            let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.int64Value ?? 0
            let sizeLabel = JourneyStyle.label(ByteCountFormatter.string(fromByteCount: size, countStyle: .file), size: 10, secondary: true)

            let remove = JourneyButton("", symbol: "xmark.circle.fill") { [weak self] in
                self?.remove(url: url)
            }
            remove.bezelStyle = .inline; remove.toolTip = "Remove attachment"
            remove.widthAnchor.constraint(equalToConstant: 14).isActive = true
            remove.heightAnchor.constraint(equalToConstant: 14).isActive = true

            for child in [icon, name, sizeLabel, remove] { pill.addArrangedSubview(child) }
            addArrangedSubview(pill)
        }
    }
}

final class ComposerView: NSTextView {
    var submit: (() -> Void)?
    var onFileDropped: ((URL) -> Void)?
    var onStepPosture: (() -> Void)?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        registerForDraggedTypes([.fileURL])
    }
    override init(frame frameRect: NSRect, textContainer: NSTextContainer?) {
        super.init(frame: frameRect, textContainer: textContainer)
        registerForDraggedTypes([.fileURL])
    }
    required init?(coder: NSCoder) {
        super.init(coder: coder)
        registerForDraggedTypes([.fileURL])
    }
    override func draggingEntered(_ sender: NSDraggingInfo) -> NSDragOperation {
        if sender.draggingPasteboard.canReadItem(withDataConformingToTypes: [NSPasteboard.PasteboardType.fileURL.rawValue]) {
            return .copy
        }
        return super.draggingEntered(sender)
    }
    override func performDragOperation(_ sender: NSDraggingInfo) -> Bool {
        if let urls = sender.draggingPasteboard.readObjects(forClasses: [NSURL.self], options: nil) as? [URL], !urls.isEmpty {
            for url in urls { onFileDropped?(url) }
            return true
        }
        return super.performDragOperation(sender)
    }
    override func paste(_ sender: Any?) {
        if let urls = NSPasteboard.general.readObjects(forClasses: [NSURL.self], options: nil) as? [URL], !urls.isEmpty {
            for url in urls { onFileDropped?(url) }
            return
        }
        super.paste(sender)
    }
    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard string.isEmpty, !hasMarkedText() else { return }
        let origin = NSPoint(x: textContainerOrigin.x + (textContainer?.lineFragmentPadding ?? 5), y: textContainerOrigin.y)
        ("Message LUMI… (⇧⇥ posture · ⌘K commands · /plan · /undo · /btw)" as NSString).draw(at: origin, withAttributes: [.font: font ?? NSFont.systemFont(ofSize: 14), .foregroundColor: NSColor.placeholderTextColor])
    }
    override func didChangeText() { super.didChangeText(); needsDisplay = true }
    override func doCommand(by selector: Selector) {
        if selector == #selector(NSResponder.insertBacktab(_:)), !hasMarkedText() {
            onStepPosture?()
            return
        }
        super.doCommand(by: selector)
    }
    override func keyDown(with event: NSEvent) {
        if event.keyCode == 36, !hasMarkedText(), !event.modifierFlags.contains(.shift) {
            submit?(); return
        }
        super.keyDown(with: event)
    }
    func dispose() { submit = nil; onFileDropped = nil; onStepPosture = nil }
}

final class TranscriptScrollView: NSScrollView {
    var beforeResize: ((Bool, NSPoint) -> Void)?
    override func setFrameSize(_ newSize: NSSize) {
        if newSize != frame.size {
            beforeResize?(documentVisibleRect.maxY >= (documentView?.bounds.height ?? 0) - 35, contentView.bounds.origin)
        }
        super.setFrameSize(newSize)
    }
}

final class TurnScrubberRail: NSView {
    struct TurnItem: Equatable {
        let index: Int
        let rowIndex: Int
        let preview: String
        let isActive: Bool
        let isError: Bool
        var isWaitingReview = false
    }
    var turns: [TurnItem] = [] {
        didSet {
            isHidden = turns.count < 2
            needsDisplay = true
        }
    }
    var onSelectTurn: ((Int) -> Void)?
    private var hoveredIndex: Int?
    private var trackingArea: NSTrackingArea?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        wantsLayer = true; translatesAutoresizingMaskIntoConstraints = false
        widthAnchor.constraint(equalToConstant: 18).isActive = true
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        if let trackingArea { removeTrackingArea(trackingArea) }
        let area = NSTrackingArea(rect: bounds, options: [.mouseMoved, .mouseEnteredAndExited, .activeInKeyWindow], owner: self, userInfo: nil)
        addTrackingArea(area)
        self.trackingArea = area
    }

    private func dotCenter(for index: Int) -> NSPoint {
        let count = turns.count
        guard count > 0 else { return .zero }
        let totalHeight = bounds.height - 24
        let step = totalHeight / CGFloat(max(1, count - 1))
        let y = bounds.height - 12 - (CGFloat(index) * min(step, 24))
        return NSPoint(x: bounds.midX, y: max(12, min(bounds.height - 12, y)))
    }

    override func draw(_ dirtyRect: NSRect) {
        super.draw(dirtyRect)
        guard turns.count >= 2 else { return }
        if let first = turns.indices.first, let last = turns.indices.last {
            let p1 = dotCenter(for: first), p2 = dotCenter(for: last)
            let path = NSBezierPath()
            path.move(to: p1); path.line(to: p2); path.lineWidth = 1.0
            NSColor.separatorColor.withAlphaComponent(0.4).setStroke()
            path.stroke()
        }
        for (i, turn) in turns.enumerated() {
            let center = dotCenter(for: i)
            let isHovered = (hoveredIndex == i)
            let radius: CGFloat = isHovered ? 4.5 : ((turn.isActive || turn.isWaitingReview) ? 4.0 : 3.0)
            let dotRect = NSRect(x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2)
            let dotPath = NSBezierPath(ovalIn: dotRect)
            let color: NSColor = turn.isError ? .systemRed : (turn.isWaitingReview ? .systemOrange : (turn.isActive ? .controlAccentColor : (isHovered ? .secondaryLabelColor : .tertiaryLabelColor)))
            color.setFill()
            dotPath.fill()
        }
    }

    override func mouseMoved(with event: NSEvent) {
        let location = convert(event.locationInWindow, from: nil)
        let previous = hoveredIndex
        hoveredIndex = turns.indices.first(where: {
            let center = dotCenter(for: $0)
            let dx = location.x - center.x, dy = location.y - center.y
            return (dx * dx + dy * dy) <= 64
        })
        if previous != hoveredIndex {
            needsDisplay = true
            if let hoveredIndex, turns.indices.contains(hoveredIndex) {
                toolTip = turns[hoveredIndex].preview
            } else { toolTip = nil }
        }
    }
    override func mouseExited(with event: NSEvent) {
        hoveredIndex = nil; toolTip = nil; needsDisplay = true
    }
    override func mouseUp(with event: NSEvent) {
        let location = convert(event.locationInWindow, from: nil)
        if let target = turns.indices.first(where: {
            let center = dotCenter(for: $0)
            let dx = location.x - center.x, dy = location.y - center.y
            return (dx * dx + dy * dy) <= 100
        }) {
            onSelectTurn?(turns[target].rowIndex)
        }
    }
    func dispose() { onSelectTurn = nil }
}

