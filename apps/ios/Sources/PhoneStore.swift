// Own configured hosts and their independent native clients; preferences contain no device tokens.
import Foundation
import Observation

@MainActor @Observable
final class PhoneStore {
    var hosts: [PhoneHostModel] = []
    var selectedHost: String?
    var error: String?
    private let defaults: UserDefaults
    private let key = "carlos.hosts.v1"

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        if let saved = defaults.data(forKey: key) {
            do {
                hosts = try JSONDecoder().decode([PhoneHost].self, from: saved).map(PhoneHostModel.init)
                selectedHost = hosts.first?.id
                for host in hosts { host.connect() }
            } catch { self.error = "Cannot read the saved host list: \(error.localizedDescription)" }
        }
    }

    var current: PhoneHostModel? { hosts.first { $0.id == selectedHost } }

    func connect(name: String, address: String, token: String, privateWebSocket: Bool) throws {
        let address = address.trimmingCharacters(in: .whitespacesAndNewlines)
        let candidate = PhoneHost(id: UUID().uuidString, name: name, address: address,
                                  allowsPrivateWebSocket: privateWebSocket)
        let url = try candidate.url()
        if let existing = hosts.first(where: { $0.configuration.address == address }) {
            try HostTokens.store(token.trimmingCharacters(in: .whitespacesAndNewlines), for: existing.id)
            selectedHost = existing.id
            existing.connect()
            return
        }
        let label = name.trimmingCharacters(in: .whitespacesAndNewlines)
        let configuration = PhoneHost(id: candidate.id, name: label.isEmpty ? (url.host ?? "Host") : label,
                                      address: address, allowsPrivateWebSocket: privateWebSocket)
        try HostTokens.store(token.trimmingCharacters(in: .whitespacesAndNewlines), for: configuration.id)
        let saved = try JSONEncoder().encode(hosts.map(\.configuration) + [configuration])
        defaults.set(saved, forKey: key)
        let host = PhoneHostModel(configuration: configuration)
        hosts.append(host)
        selectedHost = host.id
        host.connect()
    }
}
