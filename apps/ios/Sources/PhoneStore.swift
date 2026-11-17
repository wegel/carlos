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

    func beginConnection(name: String, address: String, token: String, privateWebSocket: Bool,
                         existingID: String? = nil) throws -> PhoneConnectionAttempt {
        let address = address.trimmingCharacters(in: .whitespacesAndNewlines)
        let candidate = PhoneHost(id: UUID().uuidString, name: name, address: address,
                                  allowsPrivateWebSocket: privateWebSocket)
        let url = try candidate.url()
        let existing = try connectionTarget(address: address, existingID: existingID)
        let changedAddress = existing.map { $0.configuration.connectionKey != candidate.connectionKey } ?? false
        let entered = token.trimmingCharacters(in: .whitespacesAndNewlines)
        let credential: String
        if !entered.isEmpty { credential = entered }
        else if let existing, !changedAddress { credential = try HostTokens.read(for: existing.id) }
        else { throw HostError.message("Enter a device token for this host") }
        let label = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let configuration = PhoneHost(id: existing?.id ?? candidate.id, name: label.isEmpty ? (url.host ?? "Host") : label,
                                      address: address, allowsPrivateWebSocket: privateWebSocket)
        let host: PhoneHostModel
        if let existing, !changedAddress { host = existing }
        else { host = PhoneHostModel(configuration: configuration, bodyBudget: bodyBudget, defaults: defaults, restoringSession: !changedAddress) }
        let identity = host.connect(url: url, token: credential)
        return PhoneConnectionAttempt(host: host, configuration: configuration, identity: identity,
                                      token: entered.isEmpty ? nil : credential, existingID: existingID)
    }

    func saveConnection(_ attempt: PhoneConnectionAttempt) throws -> PhoneHostModel {
        let host = attempt.host
        if let id = attempt.existingID, !hosts.contains(where: { $0.id == id }) {
            throw HostError.message("This server was removed. Add it again to reconnect.")
        }
        if let duplicate = hosts.first(where: { $0.id != host.id && $0.configuration.connectionKey == attempt.configuration.connectionKey }) {
            throw HostError.message("This address is already saved as \(duplicate.configuration.name). Edit that connection instead.")
        }
        var configurations = hosts.map { $0.id == host.id ? attempt.configuration : $0.configuration }
        let newHost = !hosts.contains { $0.id == host.id }
        if newHost { configurations.append(attempt.configuration) }
        let saved = try JSONEncoder().encode(configurations)
        try attempt.saveToken()
        defaults.set(saved, forKey: key)
        if let old = hosts.first(where: { $0.id == host.id }), old !== host {
            old.disconnect()
            defaults.removeObject(forKey: "carlos.hosts.\(host.id).session")
            hosts = hosts.map { $0.id == host.id ? host : $0 }
        }
        host.configuration = attempt.configuration
        if newHost { hosts.append(host) }
        return host
    }

    func removeHost(_ id: String) throws {
        guard let host = hosts.first(where: { $0.id == id }) else { return }
        let remaining = hosts.filter { $0.id != id }
        let saved = try JSONEncoder().encode(remaining.map(\.configuration))
        try HostTokens.remove(for: id)
        defaults.set(saved, forKey: key)
        defaults.removeObject(forKey: "carlos.hosts.\(id).session")
        host.disconnect()
        hosts = remaining
        if selectedHost == id { selectedHost = remaining.first(where: \.connected)?.id ?? remaining.last?.id }
    }

    private func connectionTarget(address: String, existingID: String?) throws -> PhoneHostModel? {
        let existing = hosts.first { $0.id == existingID }
        if existingID != nil && existing == nil {
            throw HostError.message("This server was removed. Add it again to reconnect.")
        }
        if let duplicate = hosts.first(where: { $0.id != existingID && $0.configuration.connectionKey == PhoneHost.connectionKey(address) }) {
            throw HostError.message("This address is already saved as \(duplicate.configuration.name). Edit that connection instead.")
        }
        return existing
    }
}
