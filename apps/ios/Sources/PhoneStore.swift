// Own configured hosts and their independent native clients; preferences contain no device tokens.
import Foundation
import Observation

@MainActor @Observable
final class PhoneStore {
    let preferences: PhonePreferences
    var hosts: [PhoneHostModel] = []
    var selectedHost: String? {
        didSet { defaults.set(selectedHost, forKey: "carlos.hosts.selected.v1") }
    }
    var error: String?
    private let bodyBudget = PhoneBodyBudget()
    private let defaults: UserDefaults
    private let key = "carlos.hosts.v1"

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        preferences = PhonePreferences(defaults: defaults)
        if let saved = defaults.data(forKey: key) {
            do {
                hosts = try JSONDecoder().decode([PhoneHost].self, from: saved).map { PhoneHostModel(configuration: $0, bodyBudget: bodyBudget, defaults: defaults) }
                let selected = defaults.string(forKey: "carlos.hosts.selected.v1")
                selectedHost = hosts.first(where: { $0.id == selected })?.id ?? hosts.last?.id
            } catch { self.error = "Cannot read the saved host list: \(PhoneFailureMessage.describe(error))" }
        }
    }

    var current: PhoneHostModel? { hosts.first { $0.id == selectedHost } }

    // Keychain items are available after unlock, not necessarily during app prewarming.
    func resumeConnections(force: Bool = false) {
        for host in hosts {
            switch host.link {
            case .disconnected: host.connect()
            case .connected where force: host.connect()
            default: break
            }
        }
    }

    func connect(name: String, address: String, token: String, privateWebSocket: Bool) throws -> PhoneHostModel {
        let address = address.trimmingCharacters(in: .whitespacesAndNewlines)
        let candidate = PhoneHost(id: UUID().uuidString, name: name, address: address,
                                  allowsPrivateWebSocket: privateWebSocket)
        let url = try candidate.url()
        let token = token.trimmingCharacters(in: .whitespacesAndNewlines)
        let label = name.trimmingCharacters(in: .whitespacesAndNewlines)
        if let existing = hosts.first(where: { $0.configuration.address == address }) {
            if token.isEmpty { _ = try HostTokens.read(for: existing.id) }
            else { try HostTokens.store(token, for: existing.id) }
            let configuration = PhoneHost(id: existing.id, name: label.isEmpty ? (url.host ?? "Host") : label,
                                          address: address, allowsPrivateWebSocket: privateWebSocket)
            let saved = hosts.map { $0.id == existing.id ? configuration : $0.configuration }
            defaults.set(try JSONEncoder().encode(saved), forKey: key)
            existing.configuration = configuration
            existing.connect()
            return existing
        }
        guard !token.isEmpty else { throw HostError.message("Enter a device token for this host") }
        let configuration = PhoneHost(id: candidate.id, name: label.isEmpty ? (url.host ?? "Host") : label,
                                      address: address, allowsPrivateWebSocket: privateWebSocket)
        try HostTokens.store(token, for: configuration.id)
        let saved = try JSONEncoder().encode(hosts.map(\.configuration) + [configuration])
        defaults.set(saved, forKey: key)
        let host = PhoneHostModel(configuration: configuration, bodyBudget: bodyBudget, defaults: defaults)
        hosts.append(host)
        host.connect()
        return host
    }
}
