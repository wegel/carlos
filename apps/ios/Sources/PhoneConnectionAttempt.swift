// Keep an entered credential private until this exact connection authenticates.
import Foundation

@MainActor
struct PhoneConnectionAttempt {
    let host: PhoneHostModel
    let configuration: PhoneHost
    let existingID: String?
    private let identity: UUID
    private let token: String?

    init(host: PhoneHostModel, configuration: PhoneHost, identity: UUID, token: String?, existingID: String? = nil) {
        self.host = host
        self.configuration = configuration
        self.existingID = existingID
        self.identity = identity
        self.token = token
    }

    func saveToken() throws {
        guard host.connectionIdentity == identity, host.connected else {
            throw HostError.message("This connection attempt ended. Try Connect again.")
        }
        if let token { try HostTokens.store(token, for: host.id) }
    }

    func cancel() { host.cancelConnection(identity) }
}
