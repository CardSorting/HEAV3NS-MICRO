// [LAYER: UI]
import AppKit
import Foundation

/// 5-stop interactive execution posture dial (`Shift+Tab` / `⇧⇥`).
/// Mirrors Codewhale / Claude Code / Codex CLI ergonomic permission & planning stops.
enum ExecutionPostureStop: String, CaseIterable, Equatable {
    case ask = "ask"
    case acceptEdits = "accept-edits"
    case plan = "plan"
    case autoReview = "auto-review"
    case fullAccess = "full-access"

    var label: String {
        switch self {
        case .ask: return "Ask"
        case .acceptEdits: return "Accept Edits"
        case .plan: return "Plan"
        case .autoReview: return "Auto-Review"
        case .fullAccess: return "Full Access"
        }
    }

    var symbolName: String {
        switch self {
        case .ask: return "hand.raised"
        case .acceptEdits: return "checkmark.circle"
        case .plan: return "map"
        case .autoReview: return "eye.circle"
        case .fullAccess: return "bolt.shield"
        }
    }

    var summaryHint: String {
        switch self {
        case .ask: return "Ask · prompts before file writes or shell commands (⇧⇥ to cycle)"
        case .acceptEdits: return "Accept Edits · workspace edits auto-approved; shell asks (⇧⇥)"
        case .plan: return "Plan · read-only exploration & planning without mutating tool schemas (⇧⇥)"
        case .autoReview: return "Auto-Review · autonomous inside Seatbelt sandbox; prompts on boundary risk (⇧⇥)"
        case .fullAccess: return "Full Access · autonomous execution across allowed workspace operations (⇧⇥)"
        }
    }

    func stepNext() -> ExecutionPostureStop {
        switch self {
        case .ask: return .acceptEdits
        case .acceptEdits: return .plan
        case .plan: return .autoReview
        case .autoReview: return .fullAccess
        case .fullAccess: return .ask
        }
    }

    static func parse(_ raw: String) -> ExecutionPostureStop? {
        let normalized = raw.trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .replacingOccurrences(of: "_", with: "-")
            .replacingOccurrences(of: " ", with: "-")
        switch normalized {
        case "ask", "confirm": return .ask
        case "accept-edits", "acceptedits", "edits": return .acceptEdits
        case "plan", "readonly", "read-only": return .plan
        case "auto-review", "autoreview", "review": return .autoReview
        case "full-access", "fullaccess", "autonomous", "yolo": return .fullAccess
        default: return nil
        }
    }
}

/// Honest, bounded projection of `heav3ns.receipt/v1` turn telemetry.
/// Never conflates unknown/local billing with `$0.00`.
struct TurnReceiptPresentation: Equatable {
    let turnId: String
    let postureLabel: String
    let durationMs: Int
    let filesChanged: Int
    let linesAdded: Int
    let linesRemoved: Int
    let cacheBadge: String
    let billingBadge: String
    let hasReversibleChanges: Bool
    let summaryText: String

    init?(_ raw: [String: Any]) {
        guard let turnId = raw["turn_id"] as? String ?? raw["turnId"] as? String, !turnId.isEmpty else { return nil }
        let postureRaw = (raw["posture"] as? String) ?? "auto-review"
        self.turnId = turnId
        self.postureLabel = ExecutionPostureStop.parse(postureRaw)?.label ?? postureRaw
        self.durationMs = (raw["duration_ms"] as? Int) ?? (raw["durationMs"] as? Int) ?? 0

        let changes = (raw["file_changes"] as? [[String: Any]]) ?? (raw["fileChanges"] as? [[String: Any]]) ?? []
        self.filesChanged = changes.count
        self.linesAdded = changes.reduce(0) { $0 + (($1["lines_added"] as? Int) ?? ($1["linesAdded"] as? Int) ?? 0) }
        self.linesRemoved = changes.reduce(0) { $0 + (($1["lines_removed"] as? Int) ?? ($1["linesRemoved"] as? Int) ?? 0) }

        let beforeCas = (raw["snapshotBefore"] as? String) ?? (raw["before_cas_id"] as? String) ?? (raw["beforeCasId"] as? String)
        let afterCas = (raw["snapshotAfter"] as? String) ?? (raw["after_cas_id"] as? String) ?? (raw["afterCasId"] as? String)
        self.hasReversibleChanges = !changes.isEmpty && beforeCas != nil && beforeCas != afterCas

        let cacheStatus = ((raw["prefixStatus"] as? String) ?? (raw["cache_status"] as? String) ?? (raw["cacheStatus"] as? String) ?? "stable").lowercased()
        if cacheStatus.contains("pinned") {
            self.cacheBadge = "KV pinned"
        } else if cacheStatus.contains("repin") {
            self.cacheBadge = "KV repinned"
        } else if cacheStatus.contains("drift") {
            self.cacheBadge = "KV drift"
        } else {
            self.cacheBadge = "KV stable"
        }

        if let display = (raw["billing_display"] as? String) ?? (raw["billingDisplay"] as? String), !display.isEmpty {
            self.billingBadge = display
        } else if let billing = raw["billing"] as? [String: Any], let kind = (billing["kind"] as? String)?.lowercased() {
            switch kind {
            case "local", "local_zero", "free_local":
                self.billingBadge = "local (no API charge)"
            case "subscription", "included":
                self.billingBadge = "included in plan"
            case "metered_usd", "metered":
                if let micros = billing["microdollars"] as? Int ?? billing["usd_micros"] as? Int {
                    self.billingBadge = String(format: "$%.4f", Double(micros) / 1_000_000.0)
                } else {
                    self.billingBadge = "metered"
                }
            default:
                self.billingBadge = "billing unpriced"
            }
        } else {
            self.billingBadge = "local / unpriced"
        }

        let filePart = filesChanged == 0
            ? "0 files changed"
            : "\(filesChanged) file\(filesChanged == 1 ? "" : "s") (+\(linesAdded) -\(linesRemoved))"
        let secPart = durationMs >= 100 ? String(format: "%.1fs", Double(durationMs) / 1000.0) : "\(max(1, durationMs))ms"
        self.summaryText = "Turn receipt · \(filePart) · \(cacheBadge) · \(billingBadge) · \(secPart)"
    }
}

enum RequestPreviewFormatter {
    static func format(_ data: [String: Any]) -> String {
        let schema = data["schema"] as? String ?? "heav3ns.request_manifest/v1"
        let model = data["model"] as? String ?? "unknown"
        let posture = data["posture"] as? String ?? "auto-review"
        let fp = data["prefixFingerprint"] as? [String: Any]
        let prefixHash = (fp?["combined_hash"] as? String) ?? (fp?["combinedHash"] as? String) ?? (data["prefixHash"] as? String) ?? "n/a"
        let pinStatus = (data["prefixStatus"] as? String) ?? (data["prefix_status"] as? String) ?? "pinned_initial"
        let totalBytes = (data["canonicalBodyBytes"] as? Int) ?? (data["total_canonical_bytes"] as? Int) ?? 0
        let sysBytes = (data["systemBytes"] as? Int) ?? 0
        let toolBytes = (data["toolSchemaBytes"] as? Int) ?? 0
        let histBytes = (data["historyBytes"] as? Int) ?? 0
        let frameBytes = (data["jsonFramingBytes"] as? Int) ?? 0
        let estTokens = (data["estimatedInputTokens"] as? Int) ?? (data["estimated_input_tokens"] as? Int) ?? 0
        let windowTokens = (data["contextWindowTokens"] as? Int) ?? (data["context_window_tokens"] as? Int) ?? 0
        let headroom = (data["headroomTokens"] as? Int) ?? (data["signed_headroom_tokens"] as? Int) ?? (windowTokens - estTokens)
        let activeToolNames = (data["activeTools"] as? [String]) ?? []
        let activeTools = !activeToolNames.isEmpty ? activeToolNames.count : ((data["active_tool_count"] as? Int) ?? 0)
        let msgCount = (data["messageCount"] as? Int) ?? 0

        var lines = [
            "Outbound Request Manifest (\(schema))",
            "Model: \(model)  ·  Posture: \(posture)  ·  Messages: \(msgCount)",
            "KV-Cache Prefix: fnv1a:\(prefixHash) (\(pinStatus))",
            "Canonical Payload: \(ByteCountFormatter.string(fromByteCount: Int64(totalBytes), countStyle: .file)) (\(totalBytes.formatted()) exact bytes)",
            "  • System Prefix [KV-PINNED]: \(sysBytes.formatted()) B",
            "  • Tool Schemas  [KV-PINNED]: \(toolBytes.formatted()) B (\(activeTools) active tools)",
            "  • Conversation History     : \(histBytes.formatted()) B",
            "  • JSON Envelope Framing    : \(frameBytes.formatted()) B",
            "Token Budget: ~\(estTokens.formatted()) / \(windowTokens.formatted()) tokens (signed headroom: \(headroom >= 0 ? "+" : "")\(headroom.formatted()))"
        ]
        if !activeToolNames.isEmpty {
            lines.append("\nActive Tool Schemas (\(activeToolNames.count))\n  " + activeToolNames.joined(separator: ", "))
        }
        if let preview = data["redactedSystemPreview"] as? String, !preview.isEmpty {
            lines.append("\nRedacted System Prefix Preview\n" + preview)
        }
        return lines.joined(separator: "\n")
    }
}

enum ReceiptsLedgerFormatter {
    static func format(_ data: [String: Any]) -> String {
        let receipts = data["receipts"] as? [[String: Any]] ?? []
        guard !receipts.isEmpty else {
            return "Turn Receipts Ledger (heav3ns.receipt/v1)\n\nNo turns have completed in this session yet. Send an instruction to record verifiable receipts."
        }
        var blocks = ["Turn Receipts Ledger (heav3ns.receipt/v1) · \(receipts.count) recent turn\(receipts.count == 1 ? "" : "s")"]
        for item in receipts {
            if let pres = TurnReceiptPresentation(item) {
                var detail = "• [\(pres.turnId)] \(pres.postureLabel) · \(pres.summaryText.replacingOccurrences(of: "Turn receipt · ", with: ""))"
                if let changes = item["file_changes"] as? [[String: Any]], !changes.isEmpty {
                    let paths = changes.prefix(6).compactMap { $0["path"] as? String }
                    if !paths.isEmpty { detail += "\n    Files: " + paths.joined(separator: ", ") }
                }
                blocks.append(detail)
            }
        }
        return blocks.joined(separator: "\n\n")
    }
}

/// Compact inline receipt + 1-click `/undo` bar docked cleanly above the composer.
final class TurnReceiptSummaryBar: NSStackView {
    private let iconView = NSImageView()
    private let summaryLabel = NSTextField(labelWithString: "")
    let diffButton = NSButton(title: "Changes", target: nil, action: nil)
    let receiptButton = NSButton(title: "Receipt", target: nil, action: nil)
    let undoButton = NSButton(title: "Undo Turn", target: nil, action: nil)
    private let closeButton = NSButton(title: "", target: nil, action: nil)

    var onOpenChanges: (() -> Void)?
    var onOpenReceipts: (() -> Void)?
    var onUndoTurn: (() -> Void)?

    private(set) var currentReceipt: TurnReceiptPresentation?

    override init(frame frameRect: NSRect) {
        super.init(frame: frameRect)
        orientation = .horizontal; alignment = .centerY; spacing = 8
        edgeInsets = NSEdgeInsets(top: 5, left: 10, bottom: 5, right: 8)
        wantsLayer = true; layer?.cornerRadius = 6
        layer?.backgroundColor = NSColor.controlAccentColor.withAlphaComponent(0.08).cgColor
        layer?.borderWidth = 1; layer?.borderColor = NSColor.separatorColor.cgColor
        translatesAutoresizingMaskIntoConstraints = false
        isHidden = true

        iconView.image = NSImage(systemSymbolName: "checkmark.seal", accessibilityDescription: "Turn receipt")
        iconView.contentTintColor = .controlAccentColor
        iconView.widthAnchor.constraint(equalToConstant: 14).isActive = true

        summaryLabel.font = .monospacedSystemFont(ofSize: 11, weight: .regular)
        summaryLabel.textColor = .secondaryLabelColor
        summaryLabel.lineBreakMode = .byTruncatingTail
        summaryLabel.maximumNumberOfLines = 1
        summaryLabel.setContentHuggingPriority(.init(1), for: .horizontal)
        summaryLabel.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)

        for btn in [diffButton, receiptButton, undoButton] {
            btn.bezelStyle = .inline
            btn.font = .systemFont(ofSize: 11, weight: .medium)
            btn.target = self
            btn.setContentHuggingPriority(.required, for: .horizontal)
        }
        diffButton.action = #selector(openChangesClicked); diffButton.toolTip = "Inspect modified files in Changes (⌘2)"
        receiptButton.action = #selector(openReceiptsClicked); receiptButton.toolTip = "Inspect Turn Receipts & Honest Cost Ledger"
        undoButton.action = #selector(undoClicked); undoButton.toolTip = "Safely revert files modified in the last turn (⌥⌘Z)"

        closeButton.bezelStyle = .inline
        closeButton.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Dismiss receipt summary")
        closeButton.imagePosition = .imageOnly
        closeButton.target = self; closeButton.action = #selector(dismissClicked)
        closeButton.widthAnchor.constraint(equalToConstant: 16).isActive = true

        for view in [iconView, summaryLabel, diffButton, receiptButton, undoButton, closeButton] {
            addArrangedSubview(view)
        }
    }

    required init?(coder: NSCoder) { fatalError("init(coder:) unavailable") }

    func update(with receipt: TurnReceiptPresentation?) {
        currentReceipt = receipt
        guard let receipt else {
            isHidden = true
            return
        }
        summaryLabel.stringValue = receipt.summaryText
        summaryLabel.toolTip = receipt.summaryText
        diffButton.isHidden = receipt.filesChanged == 0
        undoButton.isHidden = !receipt.hasReversibleChanges
        isHidden = false
    }

    @objc private func openChangesClicked() { onOpenChanges?() }
    @objc private func openReceiptsClicked() { onOpenReceipts?() }
    @objc private func undoClicked() { onUndoTurn?() }
    @objc private func dismissClicked() { isHidden = true }

    func dispose() {
        onOpenChanges = nil
        onOpenReceipts = nil
        onUndoTurn = nil
    }
}
