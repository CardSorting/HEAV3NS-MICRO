// [LAYER: PLUMBING]
import AppKit

struct CodeDeclaration: Equatable {
    let title: String
    let range: NSRange
}

/// A bounded lexical outline and two-tier diff highlighter for the displayed page, never presented as a language-server index.
enum CodeOutline {
    static func declarations(in text: String, path: String) -> [CodeDeclaration] {
        let ext = (path as NSString).pathExtension.lowercased()
        let pattern: String
        switch ext {
        case "swift": pattern = #"(?m)^\s*(?:(?:public|private|fileprivate|internal|open|final|static|class|override|mutating|nonisolated|indirect)\s+)*(?:struct|class|enum|protocol|extension|actor|func)\s+([\w.]+)"#
        case "ts", "tsx", "js", "jsx", "mjs", "cjs": pattern = #"(?m)^\s*(?:(?:export|default|declare|abstract|async)\s+)*(?:class|interface|type|enum|function|const|let)\s+([\w$]+)"#
        case "py": pattern = #"(?m)^\s*(?:async\s+)?(?:class|def)\s+(\w+)"#
        case "rs": pattern = #"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:extern\s+"[^"]+"\s+)?(?:struct|enum|trait|impl|fn|type|mod)\s+(\w+)"#
        case "go": pattern = #"(?m)^\s*(?:func(?:\s*\([^)]+\))?|type)\s+(\w+)"#
        default: return []
        }
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return [] }
        return regex.matches(in: text, range: NSRange(location: 0, length: (text as NSString).length)).prefix(100).map { match in
            CodeDeclaration(title: (text as NSString).substring(with: match.range).trimmingCharacters(in: .whitespacesAndNewlines), range: match.range(at: 1))
        }
    }

    static func diffSummary(_ text: String) -> (added: Int, removed: Int, files: [String]) {
        var added = 0
        var removed = 0
        var files = [String]()
        for line in text.components(separatedBy: "\n") {
            if line.hasPrefix("+++ ") {
                let raw = String(line.dropFirst(4)).trimmingCharacters(in: .whitespaces)
                let cleaned = raw.hasPrefix("b/") ? String(raw.dropFirst(2)) : raw
                if !cleaned.isEmpty && cleaned != "/dev/null" && !files.contains(cleaned) {
                    files.append(cleaned)
                }
            } else if line.hasPrefix("+") && !line.hasPrefix("+++") {
                added += 1
            } else if line.hasPrefix("-") && !line.hasPrefix("---") {
                removed += 1
            }
        }
        return (added, removed, files)
    }

    static func boundedDiffPreview(_ text: String, maxLines: Int = 18) -> (preview: String, omittedLines: Int) {
        let lines = text.components(separatedBy: "\n")
        guard lines.count > maxLines else { return (text, 0) }
        let omitted = lines.count - maxLines
        let head = lines.prefix(maxLines).joined(separator: "\n")
        return ("\(head)\n@@ … \(omitted) more line(s) — click to expand full diff @@", omitted)
    }

    /// Compresses a unified diff to only its changed (`+`/`-`) lines and concise hunk context
    /// so streaming diff cards highlight what actually changed in brief.
    static func compressedDiffPreview(_ text: String, maxChangedLines: Int = 8) -> (preview: String, omittedLines: Int, totalChangedLines: Int) {
        let lines = text.components(separatedBy: "\n")
        var out = [String]()
        var shownChanged = 0
        var totalChanged = 0
        var pendingHunkHeader: String?

        for line in lines {
            if line.hasPrefix("diff --git ") || line.hasPrefix("index ") || line.hasPrefix("--- ") || line.hasPrefix("+++ ") || line.hasPrefix("\\ ") {
                continue
            }
            if line.hasPrefix("@@") {
                if let range = line.range(of: "@@", options: .backwards), range.upperBound < line.endIndex {
                    let symbol = line[range.upperBound...].trimmingCharacters(in: .whitespaces)
                    pendingHunkHeader = symbol.isEmpty ? nil : "@@ \(symbol) @@"
                } else {
                    pendingHunkHeader = nil
                }
                continue
            }
            let isChange = (line.hasPrefix("+") && !line.hasPrefix("+++")) || (line.hasPrefix("-") && !line.hasPrefix("---"))
            guard isChange else { continue }
            totalChanged += 1
            if shownChanged < maxChangedLines {
                if let header = pendingHunkHeader {
                    out.append(header)
                    pendingHunkHeader = nil
                } else if shownChanged > 0 && out.last?.hasPrefix("@@") == false && pendingHunkHeader != nil {
                    out.append("@@ … @@")
                }
                out.append(line)
                shownChanged += 1
            }
        }

        if totalChanged == 0 {
            let fallback = boundedDiffPreview(text, maxLines: maxChangedLines)
            return (fallback.preview, fallback.omittedLines, 0)
        }
        let omitted = max(0, totalChanged - shownChanged)
        if omitted > 0 {
            out.append("@@ … \(omitted) more changed line(s) — click to expand full diff @@")
        }
        return (out.joined(separator: "\n"), omitted, totalChanged)
    }

    static func highlight(_ text: String, diff: Bool, fontSize: CGFloat = 12) -> NSAttributedString {
        let baseFont = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .regular)
        let boldFont = NSFont.monospacedSystemFont(ofSize: fontSize, weight: .semibold)
        let result = NSMutableAttributedString(string: text, attributes: [.font: baseFont, .foregroundColor: NSColor.labelColor])
        if !diff {
            let rules: [(String, NSColor)] = [
                (#"\b(?:import|from|use|crate|mod|pub|fn|impl|trait|match|mut|where|for|in|loop|while|class|struct|enum|protocol|func|function|async|await|return|let|var|const|if|else|guard|private|public|export|type|interface|def|self|Self|nil|true|false|Some|None|Ok|Err)\b"#, .systemPurple),
                (#"\"(?:[^\"\\]|\\.)*\""#, .systemRed),
                (#"(?m)//[^\n]*"#, .secondaryLabelColor)
            ]
            for (pattern, color) in rules {
                guard let regex = try? NSRegularExpression(pattern: pattern) else { continue }
                for match in regex.matches(in: text, range: NSRange(location: 0, length: result.length)) {
                    result.addAttribute(.foregroundColor, value: color, range: match.range)
                }
            }
            return result
        }

        let lines = text.components(separatedBy: "\n")
        let diffParagraph = NSMutableParagraphStyle()
        diffParagraph.lineSpacing = 2.5
        diffParagraph.headIndent = 14
        diffParagraph.firstLineHeadIndent = 0
        result.addAttribute(.paragraphStyle, value: diffParagraph, range: NSRange(location: 0, length: result.length))
        var lineOffsets = [Int]()
        lineOffsets.reserveCapacity(lines.count)
        var currentOffset = 0
        for line in lines {
            lineOffsets.append(currentOffset)
            currentOffset += (line as NSString).length + 1
        }

        func applyIntraLineHighlight(oldLine: String, oldOffset: Int, newLine: String, newOffset: Int) {
            let oldSlice = Array((oldLine as NSString).substring(from: 1).utf16)
            let newSlice = Array((newLine as NSString).substring(from: 1).utf16)
            var p = 0
            while p < oldSlice.count && p < newSlice.count && oldSlice[p] == newSlice[p] { p += 1 }
            var s = 0
            while s < (oldSlice.count - p) && s < (newSlice.count - p) && oldSlice[oldSlice.count - 1 - s] == newSlice[newSlice.count - 1 - s] { s += 1 }
            let oldChangeLen = oldSlice.count - p - s
            if oldChangeLen > 0 {
                let r = NSRange(location: oldOffset + 1 + p, length: oldChangeLen)
                result.addAttribute(.backgroundColor, value: NSColor.systemRed.withAlphaComponent(0.32), range: r)
                result.addAttribute(.font, value: boldFont, range: r)
            }
            let newChangeLen = newSlice.count - p - s
            if newChangeLen > 0 {
                let r = NSRange(location: newOffset + 1 + p, length: newChangeLen)
                result.addAttribute(.backgroundColor, value: NSColor.systemGreen.withAlphaComponent(0.35), range: r)
                result.addAttribute(.font, value: boldFont, range: r)
            }
        }

        var i = 0
        while i < lines.count {
            let line = lines[i]
            let offset = lineOffsets[i]
            let lineRange = NSRange(location: offset, length: (line as NSString).length)

            if line.hasPrefix("diff --git ") || line.hasPrefix("--- ") || line.hasPrefix("+++ ") || line.hasPrefix("index ") {
                result.addAttribute(.foregroundColor, value: NSColor.secondaryLabelColor, range: lineRange)
                result.addAttribute(.font, value: boldFont, range: lineRange)
                result.addAttribute(.backgroundColor, value: NSColor.labelColor.withAlphaComponent(0.05), range: lineRange)
                i += 1
            } else if line.hasPrefix("@@ …") {
                result.addAttribute(.foregroundColor, value: NSColor.secondaryLabelColor, range: lineRange)
                result.addAttribute(.font, value: NSFont.monospacedSystemFont(ofSize: max(10, fontSize - 0.5), weight: .medium), range: lineRange)
                result.addAttribute(.backgroundColor, value: NSColor.labelColor.withAlphaComponent(0.04), range: lineRange)
                i += 1
            } else if line.hasPrefix("@@") {
                result.addAttribute(.foregroundColor, value: NSColor.systemBlue, range: lineRange)
                result.addAttribute(.font, value: boldFont, range: lineRange)
                result.addAttribute(.backgroundColor, value: NSColor.systemBlue.withAlphaComponent(0.10), range: lineRange)
                i += 1
            } else if line.hasPrefix("\\ ") {
                result.addAttribute(.foregroundColor, value: NSColor.tertiaryLabelColor, range: lineRange)
                i += 1
            } else if line.hasPrefix("-") && !line.hasPrefix("---") {
                var delEnd = i
                while delEnd < lines.count && lines[delEnd].hasPrefix("-") && !lines[delEnd].hasPrefix("---") {
                    let r = NSRange(location: lineOffsets[delEnd], length: (lines[delEnd] as NSString).length)
                    result.addAttribute(.foregroundColor, value: NSColor.systemRed, range: r)
                    result.addAttribute(.backgroundColor, value: NSColor.systemRed.withAlphaComponent(0.09), range: r)
                    if r.length > 0 { result.addAttribute(.font, value: boldFont, range: NSRange(location: r.location, length: 1)) }
                    delEnd += 1
                }
                var addEnd = delEnd
                while addEnd < lines.count && lines[addEnd].hasPrefix("+") && !lines[addEnd].hasPrefix("+++") {
                    let r = NSRange(location: lineOffsets[addEnd], length: (lines[addEnd] as NSString).length)
                    result.addAttribute(.foregroundColor, value: NSColor.systemGreen, range: r)
                    result.addAttribute(.backgroundColor, value: NSColor.systemGreen.withAlphaComponent(0.09), range: r)
                    if r.length > 0 { result.addAttribute(.font, value: boldFont, range: NSRange(location: r.location, length: 1)) }
                    addEnd += 1
                }
                let pairedCount = min(delEnd - i, addEnd - delEnd)
                if pairedCount > 0 && pairedCount <= 16 {
                    for k in 0..<pairedCount {
                        applyIntraLineHighlight(
                            oldLine: lines[i + k], oldOffset: lineOffsets[i + k],
                            newLine: lines[delEnd + k], newOffset: lineOffsets[delEnd + k]
                        )
                    }
                }
                i = addEnd
            } else if line.hasPrefix("+") && !line.hasPrefix("+++") {
                result.addAttribute(.foregroundColor, value: NSColor.systemGreen, range: lineRange)
                result.addAttribute(.backgroundColor, value: NSColor.systemGreen.withAlphaComponent(0.09), range: lineRange)
                if lineRange.length > 0 { result.addAttribute(.font, value: boldFont, range: NSRange(location: lineRange.location, length: 1)) }
                i += 1
            } else {
                i += 1
            }
        }
        return result
    }

    /// Highlights a live-streaming diff prefix without polluting intra-line diffing,
    /// washes the active write-head line, and appends a styled teal editor caret (`▌`).
    static func streamingDiffAttributed(_ slice: String, hasMore: Bool, fontSize: CGFloat = 11) -> NSAttributedString {
        let highlighted = NSMutableAttributedString(attributedString: highlight(slice, diff: true, fontSize: fontSize))
        guard hasMore else { return highlighted }
        let nsSlice = slice as NSString
        if nsSlice.length > 0 {
            let lastNewline = nsSlice.range(of: "\n", options: .backwards)
            let lineStart = lastNewline.location == NSNotFound ? 0 : lastNewline.location + 1
            let activeRange = NSRange(location: lineStart, length: nsSlice.length - lineStart)
            if activeRange.length > 0 {
                highlighted.addAttribute(.backgroundColor, value: NSColor.systemTeal.withAlphaComponent(0.11), range: activeRange)
            }
        }
        let caret = NSAttributedString(
            string: "▌",
            attributes: [
                .font: NSFont.monospacedSystemFont(ofSize: fontSize + 0.5, weight: .bold),
                .foregroundColor: NSColor.systemTeal
            ]
        )
        highlighted.append(caret)
        return highlighted
    }
}
