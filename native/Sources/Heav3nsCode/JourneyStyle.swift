// [LAYER: PLUMBING]
import AppKit

/// Native bindings for the approved journey. AppKit retains focus, selection and appearance behavior.
enum JourneyStyle {
    static func label(_ text: String, size: CGFloat = 13, weight: NSFont.Weight = .regular, secondary: Bool = false) -> NSTextField {
        let label = NSTextField(wrappingLabelWithString: text)
        label.font = .systemFont(ofSize: size, weight: weight)
        label.textColor = secondary ? .secondaryLabelColor : .labelColor
        return label
    }
    static func stack(_ views: [NSView], vertical: Bool = true, spacing: CGFloat = 12) -> NSStackView {
        let stack = NSStackView(views: views)
        stack.orientation = vertical ? .vertical : .horizontal
        stack.alignment = vertical ? .leading : .centerY
        stack.spacing = spacing; stack.translatesAutoresizingMaskIntoConstraints = false
        return stack
    }
    static func rule() -> NSBox { let line = NSBox(); line.boxType = .separator; return line }
    static func pin(_ view: NSView, to parent: NSView, inset: CGFloat = 0) {
        view.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            view.leadingAnchor.constraint(equalTo: parent.leadingAnchor, constant: inset),
            view.trailingAnchor.constraint(equalTo: parent.trailingAnchor, constant: -inset),
            view.topAnchor.constraint(equalTo: parent.topAnchor, constant: inset),
            view.bottomAnchor.constraint(equalTo: parent.bottomAnchor, constant: -inset),
        ])
    }
    /// Clamps a window's size and origin to the active screen's `visibleFrame` so it never extends past screen edges.
    static func fitWindow(_ window: NSWindow, desiredSize: NSSize? = nil, margin: CGFloat = 24, center: Bool = true) {
        guard let visible = (window.screen ?? NSScreen.main ?? NSScreen.screens.first)?.visibleFrame else { return }
        let maxW = max(320, visible.width - margin)
        let maxH = max(240, visible.height - margin)
        if window.minSize.width > maxW || window.minSize.height > maxH {
            window.minSize = NSSize(width: min(window.minSize.width, maxW), height: min(window.minSize.height, maxH))
        }
        var frame = window.frame
        if let desiredSize {
            frame.size = NSSize(width: min(desiredSize.width, maxW), height: min(desiredSize.height, maxH))
        } else {
            frame.size = NSSize(width: min(frame.width, maxW), height: min(frame.height, maxH))
        }
        if center {
            frame.origin = NSPoint(x: visible.midX - frame.width / 2, y: visible.midY - frame.height / 2)
        } else {
            frame.origin.x = max(visible.minX, min(frame.origin.x, visible.maxX - frame.width))
            frame.origin.y = max(visible.minY, min(frame.origin.y, visible.maxY - frame.height))
        }
        window.setFrame(frame, display: true)
    }
}

final class JourneyButton: NSButton {
    private let handler: () -> Void
    init(_ title: String, symbol: String? = nil, action: @escaping () -> Void) {
        handler = action
        super.init(frame: .zero)
        self.title = title; target = self; self.action = #selector(activate)
        bezelStyle = .rounded; controlSize = .regular
        if let symbol { image = NSImage(systemSymbolName: symbol, accessibilityDescription: nil); imagePosition = .imageLeading }
        setAccessibilityLabel(title)
    }
    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }
    @objc private func activate() { handler() }
    func dispose() { target = nil; action = nil }
}
