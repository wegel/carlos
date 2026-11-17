// Use the actual row width to keep short reasoning and tools in the demo's tight groups.
import UIKit

enum PhoneEntryLayout {
    case hidden, compact, block

    @MainActor
    static func presentation(_ row: PhoneEntry, width: CGFloat) -> Self {
        if row.tool != nil { return .compact }
        guard row.kind == .reasoning else { return .block }
        if row.text.sourceBytes != nil { return .block }
        let text = row.text.preview
        if text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return row.done ? .hidden : .compact
        }
        guard text.rangeOfCharacter(from: .newlines) == nil else { return .block }
        let font = UIFont(name: "InterVariableItalic", size: 13.5) ?? .italicSystemFont(ofSize: 13.5)
        let measured = (text as NSString).size(withAttributes: [.font: font]).width
        return measured <= max(0, width - 24) ? .compact : .block
    }

    static func toolLabel(_ tool: PhoneTool, glyph: String) -> String {
        // Old saved rows can already include the same decorative glyph as this native column.
        let prefix = glyph + " "
        return tool.labelSource != .ai && tool.label.hasPrefix(prefix)
            ? String(tool.label.dropFirst(prefix.count)) : tool.label
    }
}
