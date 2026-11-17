// Track one device's in-flight history read; Rust owns every cursor and accepted row.
import Foundation
import Observation

@MainActor @Observable
final class PhoneHistoryModel {
    private(set) var loading = false
    private(set) var error: String?
    @ObservationIgnored private var attempt = UUID()
    @ObservationIgnored private var request: Int64?
    @ObservationIgnored private var first: String?

    func load(host: PhoneHostModel) {
        guard !loading, host.connected, host.transcript?.currentSnapshot == true,
              host.transcript?.hasOlder == true, let session = host.selected,
              let first = host.orderedEntries.first else { return }
        loading = true
        error = nil
        self.first = first
        let identity = UUID()
        attempt = identity
        host.transport.request({ try $0.olderHistory(session: session) }) { [weak self] result in
            guard let self, attempt == identity else { return }
            switch result {
            case .success(let frame): request = frame.requestId
            case .failure(let failure): fail(PhoneFailureMessage.describe(failure))
            }
        }
    }

    func receive(_ batch: PhoneBatch) -> Int64? {
        guard let request, let failure = batch.failures.first(where: { $0.requestId == request }) else { return nil }
        fail(failure.message)
        return request
    }

    func latest(host: PhoneHostModel) {
        guard host.connected, host.transcript?.hasNewer == true, let session = host.selected else { return }
        reset()
        host.transcript?.currentSnapshot = false
        host.transcript?.ready = false
        host.transport.command({ try $0.subscribe(session: session) }) { [weak host] result in
            guard let host, host.selected == session else { return }
            if case .failure(let failure) = result { host.error = PhoneFailureMessage.describe(failure) }
        }
    }

    func updated(_ transcript: PhoneTranscript, first: String?) {
        if loading && (first != self.first || !transcript.hasOlder) { reset() }
    }

    func reset() {
        attempt = UUID()
        request = nil
        first = nil
        loading = false
        error = nil
    }

    private func fail(_ message: String) {
        reset()
        error = message
    }
}
