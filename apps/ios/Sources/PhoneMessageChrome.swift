// Share entry headings and full-width user bands between checked rows and this device's private prompt echo.
import SwiftUI

extension PhoneEntryKind {
    var glyph: String {
        switch self { case .user: "❯"; case .reasoning: "◆"; case .commentary: "•"; case .answer: "✓"; case .notice: ""; case .toolCall, .tool: "$" }
    }

    var caption: String {
        switch self { case .user: "you"; case .reasoning: "reasoning"; case .commentary: "commentary"; case .answer: "answer"; case .notice: "notice"; case .toolCall, .tool: "tool" }
    }

    var textColor: Color {
        switch self {
        case .user, .answer: PhoneTheme.text
        case .reasoning: PhoneTheme.mauve
        case .commentary: PhoneTheme.flamingo
        case .notice: PhoneTheme.overlay2
        case .toolCall, .tool: PhoneTheme.subtext1
        }
    }

    var accent: Color { self == .user ? PhoneTheme.blue : self == .answer ? PhoneTheme.green : textColor }
}

struct PhoneEntryHeading: View {
    let kind: PhoneEntryKind

    var body: some View {
        HStack(spacing: 8) {
            Rectangle().frame(width: 10, height: 1).opacity(0.4)
            Text(kind.glyph).bold()
            Text(kind.caption)
            Rectangle().frame(height: 1).opacity(0.4)
        }.font(PhoneTheme.mono(12)).foregroundStyle(kind.accent)
    }
}

struct PhoneUserMessage<Content: View>: View {
    @ViewBuilder let content: Content
    @Environment(PhonePreferences.self) private var preferences

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if preferences.layout == .flow { PhoneEntryHeading(kind: .user) }
            HStack(alignment: .top, spacing: 8) {
                if preferences.layout == .gutter {
                    Text(PhoneEntryKind.user.glyph).font(PhoneTheme.mono(13)).bold()
                        .foregroundStyle(PhoneTheme.blue).frame(width: 14)
                }
                content.frame(maxWidth: .infinity, alignment: .leading)
            }
            .padding(.vertical, 12)
            .background {
                GeometryReader { geometry in
                    PhoneTheme.userBand.frame(width: geometry.size.width + 32).offset(x: -16)
                }
            }
        }
    }
}
