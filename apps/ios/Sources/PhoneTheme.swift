// Use the phone spec's bundled fonts and Catppuccin Mocha colors in one place.
import SwiftUI

enum PhoneTheme {
    static let crust = color(0x11111b)
    static let mantle = color(0x181825)
    static let base = color(0x1e1e2e)
    static let surface0 = color(0x313244)
    static let surface1 = color(0x45475a)
    static let userBand = color(0x1e2640)
    static let approval = color(0x1f1e27)
    static let overlay1 = color(0x7f849c)
    static let overlay2 = color(0x9399b2)
    static let subtext0 = color(0xa6adc8)
    static let subtext1 = color(0xbac2de)
    static let text = color(0xcdd6f4)
    static let blue = color(0x89b4fa)
    static let mauve = color(0xcba6f7)
    static let pink = color(0xf5c2e7)
    static let flamingo = color(0xf2cdcd)
    static let green = color(0xa6e3a1)
    static let red = color(0xf38ba8)
    static let yellow = color(0xf9e2af)
    static let peach = color(0xfab387)
    static let teal = color(0x94e2d5)

    static func prose(_ size: CGFloat = 15, weight: Font.Weight = .regular) -> Font {
        .custom("InterVariable", size: size).weight(weight)
    }

    static func mono(_ size: CGFloat = 12) -> Font {
        .custom("JetBrainsMono-Regular", size: size)
    }

    private static func color(_ rgb: UInt32) -> Color {
        Color(red: Double((rgb >> 16) & 255) / 255,
              green: Double((rgb >> 8) & 255) / 255, blue: Double(rgb & 255) / 255)
    }
}

extension PhoneBackend {
    var label: String {
        switch self { case .codex: "codex"; case .claude: "claude"; case .grok: "grok" }
    }
}

extension PhoneSession {
    var title: String { name.flatMap { $0.isEmpty ? nil : $0 } ?? "New session" }
    var subtitle: String { [backend.label, host, cwd].compactMap { $0 }.joined(separator: " · ") }
    var status: String {
        switch activity.availability {
        case .closed: return "closed"
        case .unavailable: return "unavailable"
        case .dormant: return "saved"
        case .starting, .restoring: return "starting"
        case .ready: break
        }
        switch activity.phase {
        case .approval: return "needs approval"
        case .running: return activity.ralph ? "ralph" : "running"
        case .idle: return "idle"
        }
    }
    var statusColor: Color {
        switch activity.phase {
        case .approval: PhoneTheme.yellow
        case .running: activity.ralph ? PhoneTheme.pink : PhoneTheme.mauve
        case .idle: PhoneTheme.overlay2
        }
    }
}
