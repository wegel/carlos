// Read one immutable saved prompt; a separate draft survives cancellation and failed branch creation.
import Foundation
import Observation

@MainActor @Observable
final class PhonePromptEditor: Identifiable {
    let id = UUID()
    let session: String
    let entry: String
    var text = ""
    private(set) var loaded = false
    private(set) var loading = false
    private(set) var sending = false
    private(set) var error: String?
    private(set) var operation: PhoneOperation?
    @ObservationIgnored private let transport: PhoneTransport
    @ObservationIgnored private var attempt = UUID()
    @ObservationIgnored private var request: Int64?
    @ObservationIgnored private var prompt: PhonePrompt?
    @ObservationIgnored private var original = ""
    @ObservationIgnored private var receivedBytes: UInt64 = 0

    init(session: String, entry: String, transport: PhoneTransport) {
        self.session = session
        self.entry = entry
        self.transport = transport
    }

    func load() {
        guard !loaded, !sending else { return }
        attempt = UUID()
        prompt = nil
        original = ""
        receivedBytes = 0
        error = nil
        loading = true
        let session = session, entry = entry
        issue { try $0.selectPromptForEntry(session: session, entry: entry) }
    }

    func receive(_ batch: PhoneBatch) -> Int64? {
        guard let request else { return nil }
        if let failure = batch.failures.first(where: { $0.requestId == request }) {
            fail(failure.message)
            return request
        }
        for reply in batch.prompts {
            switch reply {
            case .selected(let id, let session, let prompt) where id == request && session == self.session:
                guard let prompt, prompt.entry == entry else { fail("This message is no longer editable."); return nil }
                self.prompt = prompt
                readNext(offset: 0)
            case .text(let id, let session, let prompt, let offset, let total, let text, let next)
                where id == request && session == self.session && prompt == self.prompt?.id:
                guard offset == receivedBytes, total == self.prompt?.bytes else {
                    fail("The original message changed. Close and choose it again."); return nil
                }
                original += text
                receivedBytes += UInt64(text.utf8.count)
                if let next { readNext(offset: next) }
                else {
                    self.text = original
                    original = ""
                    loaded = true
                    loading = false
                    self.request = nil
                }
            default: break
            }
        }
        return nil
    }

    func submit(host: PhoneHostModel) {
        guard loaded, !sending, operation == nil, let prompt, host.selected == session,
              host.connected, host.transcript?.ready == true,
              host.transcript?.info.activity.phase == .idle,
              !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        sending = true
        error = nil
        let session = session, id = prompt.id, text = text
        transport.submit({ try $0.rewindPrompt(session: session, prompt: id, text: text) }) { [weak self, weak host] result in
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
        if loading { fail("Connection lost while reading. Your draft is kept.") }
        if sending {
            sending = false
            error = "Connection lost. Your edited text is kept; check the outcome before sending again."
        }
    }

    private func readNext(offset: UInt64) {
        guard let prompt else { return }
        let session = session, id = prompt.id
        issue { try $0.readPromptText(session: session, prompt: id, offset: offset) }
    }

    private func issue(_ call: @escaping @Sendable (PhoneClient) throws -> PhoneFrame) {
        request = nil
        let identity = attempt
        transport.request(call) { [weak self] result in
            guard let self, attempt == identity else { return }
            switch result {
            case .success(let frame): request = frame.requestId
            case .failure(let failure): fail(PhoneFailureMessage.describe(failure))
            }
        }
    }

    private func fail(_ message: String) {
        attempt = UUID()
        request = nil
        loading = false
        error = message
    }
}
