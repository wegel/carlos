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

    func beginConnection(name: String, address: String, token: String, privateWebSocket: Bool) throws -> PhoneConnectionAttempt {
        let address = address.trimmingCharacters(in: .whitespacesAndNewlines)
        let candidate = PhoneHost(id: UUID().uuidString, name: name, address: address,
                                  allowsPrivateWebSocket: privateWebSocket)
        let url = try candidate.url()
        let existing = hosts.first { $0.configuration.address == address }
        let entered = token.trimmingCharacters(in: .whitespacesAndNewlines)
        let credential: String
        if !entered.isEmpty { credential = entered }
        else if let existing { credential = try HostTokens.read(for: existing.id) }
        else { throw HostError.message("Enter a device token for this host") }
        let label = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let configuration = PhoneHost(id: existing?.id ?? candidate.id, name: label.isEmpty ? (url.host ?? "Host") : label,
                                      address: address, allowsPrivateWebSocket: privateWebSocket)
        let host = existing ?? PhoneHostModel(configuration: configuration, bodyBudget: bodyBudget, defaults: defaults)
        let identity = host.connect(url: url, token: credential)
        return PhoneConnectionAttempt(host: host, configuration: configuration, identity: identity, token: entered.isEmpty ? nil : credential)
    }

    func saveConnection(_ attempt: PhoneConnectionAttempt) throws -> PhoneHostModel {
        let host = attempt.host
        var configurations = hosts.map { $0.id == host.id ? attempt.configuration : $0.configuration }
        let newHost = !hosts.contains { $0.id == host.id }
        if newHost { configurations.append(attempt.configuration) }
        let saved = try JSONEncoder().encode(configurations)
        try attempt.saveToken()
        defaults.set(saved, forKey: key)
        host.configuration = attempt.configuration
        if newHost { hosts.append(host) }
        return host
    }
}
