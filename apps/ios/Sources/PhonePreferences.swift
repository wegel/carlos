// Keep display choices on this device; no preference changes shared session facts.
import Foundation
import Observation

enum PhoneToolMode: String, CaseIterable, Identifiable {
    case auto = "Auto", compact = "Compact", full = "Full"
    var id: String { rawValue }
}

enum PhoneLayout: String, CaseIterable, Identifiable {
    case flow = "Flow", gutter = "Gutter"
    var id: String { rawValue }
}

@MainActor @Observable
final class PhonePreferences {
    var toolMode: PhoneToolMode {
        didSet {
            defaults.set(toolMode.rawValue, forKey: "carlos.tools.mode")
            // A new global mode takes effect immediately; later row taps override it.
            overrides = [:]
            saveOverrides()
        }
    }
    var layout: PhoneLayout {
        didSet { defaults.set(layout.rawValue, forKey: "carlos.transcript.layout") }
    }
    private var overrides: [String: Bool]
    @ObservationIgnored private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        toolMode = defaults.string(forKey: "carlos.tools.mode").flatMap(PhoneToolMode.init) ?? .auto
        layout = defaults.string(forKey: "carlos.transcript.layout").flatMap(PhoneLayout.init) ?? .flow
        overrides = defaults.dictionary(forKey: "carlos.tools.overrides") as? [String: Bool] ?? [:]
    }

    func isExpanded(host: String, session: String, entry: String) -> Bool {
        overrides[key(host, session, entry)] ?? (toolMode == .full)
    }

    func toggle(host: String, session: String, entry: String) {
        let address = key(host, session, entry)
        overrides[address] = !isExpanded(host: host, session: session, entry: entry)
        saveOverrides()
    }

    private func key(_ host: String, _ session: String, _ entry: String) -> String {
        "\(host)/\(session)/\(entry)"
    }

    private func saveOverrides() { defaults.set(overrides, forKey: "carlos.tools.overrides") }
}
