// Read only an expanded tool's checked, bounded output lifetime; original copy bytes stay separate.
import Foundation
import Observation

@MainActor @Observable
final class PhoneOutputModel {
    let session: String
    let entry: String
    var window: PhoneOutputWindow?
    var version = UUID()
    var error: String?
    @ObservationIgnored private let transport: PhoneTransport
    @ObservationIgnored private var body: PhoneBody?
    @ObservationIgnored private var active = true
    @ObservationIgnored private var opening = false
    @ObservationIgnored private var reading = false
    @ObservationIgnored private var watching = false
    @ObservationIgnored private var suspended = false

    init(session: String, entry: String, transport: PhoneTransport) {
        self.session = session
        self.entry = entry
        self.transport = transport
    }

    func start() {
        guard active, !suspended, !opening, body == nil else { return }
        opening = true
        error = nil
        let session = session, entry = entry
        transport.read({ try $0.openOutput(session: session, entry: entry) }) { [weak self, transport] result in
            self?.opening = false
            switch result {
            case .success(let body):
                guard let self, active else { transport.batch({ try $0.closeBody(body: body) }) { _ in }; return }
                self.body = body
                readNext()
            case .failure(let failure): self?.error = failure.localizedDescription
            }
        }
    }

    func refresh(_ current: PhoneBody) {
        guard active, !suspended, current.session() == session, current.entry() == entry, !current.isSource(),
              let body else { return }
        if !current.sameRead(other: body) {
            self.body = current
            reading = false
            watching = false
        }
        refresh()
    }

    func suspend() { suspended = true; reading = false }

    func resume() {
        guard active, suspended else { return }
        suspended = false
        error = nil
        if body == nil { start() } else { refresh() }
    }

    func close() {
        active = false
        if let body { transport.batch({ try $0.closeBody(body: body) }) { _ in } }
        body = nil
        window = nil
    }

    func retry() {
        guard active, !suspended else { return }
        if let body { transport.batch({ try $0.closeBody(body: body) }) { _ in } }
        body = nil
        reading = false
        watching = false
        start()
    }

    private func refresh() {
        guard active, !suspended, let body else { return }
        reading = false
        transport.read({ try $0.outputWindow(body: body) }) { [weak self] result in
            guard let self, active, !suspended, self.body?.sameRead(other: body) == true else { return }
            switch result {
            case .success(let window):
                self.window = window
                version = UUID()
                error = nil
                if window.hasMore { if !watching { readNext() } }
                else if !window.done { watch() }
            case .failure(let failure):
                if case PhoneError.StaleRead = failure {
                    self.body = nil
                    watching = false
                    start()
                } else { error = failure.localizedDescription }
            }
        }
    }

    private func readNext() {
        guard active, !suspended, !reading, let body else { return }
        reading = true
        transport.command({ try $0.readBody(body: body) }) { [weak self] result in
            if case .failure(let failure) = result { self?.reading = false; self?.error = failure.localizedDescription }
        }
    }

    private func watch() {
        guard active, !suspended, !watching, let body else { return }
        watching = true
        transport.command({ try $0.watchBody(body: body) }) { [weak self] result in
            if case .failure(let failure) = result { self?.watching = false; self?.error = failure.localizedDescription }
        }
    }
}
