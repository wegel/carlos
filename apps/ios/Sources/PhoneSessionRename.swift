// Keep a private name until the daemon confirms its single explicit rename request.
import Foundation
import Observation

@MainActor @Observable
final class PhoneSessionRename: Identifiable {
    let id = UUID()
    let session: String
    var name: String
    private(set) var sending = false
    private(set) var error: String?
    private(set) var operation: PhoneOperation?
    @ObservationIgnored private let transport: PhoneTransport

    init(session: PhoneSession, transport: PhoneTransport) {
        self.session = session.id
        name = session.name ?? ""
        self.transport = transport
    }

    var uncertain: Bool {
        if case .uncertain = operation?.state { return true }
        return false
    }

    func submit(host: PhoneHostModel) {
        guard !sending, !uncertain, host.connected, host.selected == session,
              host.transcript?.ready == true else { return }
        sending = true
        error = nil
        let session = session, name = name
        transport.submit({ try $0.renameSession(session: session, name: name) }) { [weak self, weak host] result in
            guard let self else { return }
            switch result {
            case .success(let operation):
                self.operation = operation
                host?.accept(operation)
            case .failure(let failure):
                sending = false
                error = PhoneFailureMessage.describe(failure)
            }
        }
    }

    func observe(_ operation: PhoneOperation) -> Bool {
        guard operation.key == self.operation?.key else { return false }
        self.operation = operation
        switch operation.state {
        case .accepted: sending = false; return true
        case .rejected(let message), .uncertain(let message): sending = false; error = message
        default: break
        }
        return false
    }

    func disconnected() {
        if sending {
            sending = false
            error = "Connection lost. Your name is kept; check the session before trying again."
        }
    }
}
