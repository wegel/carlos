// Keep each host's device-local draft and visible facts outside the shared Rust session.
import Foundation
import Observation

@MainActor @Observable
final class PhoneHostModel: Identifiable {
    nonisolated let configuration: PhoneHost
    nonisolated var id: String { configuration.id }
    var link: PhoneLinkState = .disconnected(nil)
    var catalog: [PhoneSession] = []
    var selected: String?
    var transcript: PhoneTranscript?
    var orderedEntries: [String] = []
    var rows: [String: PhoneEntry] = [:]
    var operations: [String: PhoneOperation] = [:]
    var operationOrder: [String] = []
    var approvals: [PhoneApproval] = []
    var ralphRequest: PhoneOperation?
    var desiredRalph: Bool?
    var contentVersion = UUID()
    var followRequest = UUID()
    var controlSubmitting = false
    var draft = ""
    var submitting = false
    var creating = false
    var error: String?
    @ObservationIgnored let transport = PhoneTransport()
    @ObservationIgnored private var catalogRequested = false
    @ObservationIgnored private var creation: String?
    @ObservationIgnored private let bodyBudget: PhoneBodyBudget
    @ObservationIgnored private var sources: [String: PhoneSourceModel] = [:]

    init(configuration: PhoneHost, bodyBudget: PhoneBodyBudget = PhoneBodyBudget()) {
        self.configuration = configuration
        self.bodyBudget = bodyBudget
    }

    func contentChanged() { contentVersion = UUID() }

    func loadedSource(for entry: String) -> PhoneSourceModel? { sources[entry] }

    func source(for entry: String) -> PhoneSourceModel? {
        guard let selected else { return nil }
        if let source = sources[entry] { return source }
        let kind = rows[entry]?.kind
        let source = PhoneSourceModel(session: selected, entry: entry, transport: transport, budget: bodyBudget, markdown: kind == .answer || kind == .commentary, changed: { [weak self] in self?.contentChanged() })
        sources[entry] = source
        return source
    }


    var connected: Bool {
        if case .connected = link { return true }
        return false
    }

    func connect() {
        do {
            let token = try HostTokens.read(for: id)
            let callbacks = PhoneCallbacks(
                received: { [weak self] batch in self?.received(batch) },
                link: { [weak self] state in self?.changedLink(state) }
            )
            transport.connect(url: try configuration.url(), token: token, callbacks: callbacks)
        } catch { self.error = error.localizedDescription }
    }

    func select(_ session: String) {
        guard connected else { error = "Reconnect to choose a session"; return }
        draft = ""
        guard selected != session else { return }
        let old = selected
        selected = session
        for source in sources.values { source.close() }
        sources = [:]
        transcript = nil
        orderedEntries = []
        rows = [:]
        approvals = []
        if let old, old != session {
            transport.command({ try $0.unsubscribe(session: old) }, completion: report)
        }
        transport.command({ try $0.subscribe(session: session) }, completion: report)
    }

    func create(backend: PhoneBackend, cwd: String) {
        guard !creating, connected else { return }
        creating = true
        transport.submit({ try $0.createSession(backend: backend, cwd: cwd) }) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let operation):
                creation = operation.key
                accept(operation)
            case .failure(let failure):
                creating = false
                error = failure.localizedDescription
            }
        }
    }

    func send() {
        guard let selected, !submitting, connected, transcript?.ready == true,
              !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        let text = draft
        submitting = true
        transport.submit({ try $0.sendPrompt(session: selected, text: text) }) { [weak self] result in
            guard let self else { return }
            submitting = false
            switch result {
            case .success(let operation):
                accept(operation)
                if self.selected == selected {
                    followRequest = UUID()
                    if draft == text { draft = "" }
                }
            case .failure(let failure): error = failure.localizedDescription
            }
        }
    }

    func interrupt() {
        guard let selected, connected, !controlSubmitting else { return }
        submitControl { try $0.interrupt(session: selected) }
    }

    func approve(_ card: PhoneApproval, decision: PhoneDecision) {
        guard let selected, connected, !controlSubmitting else { return }
        followRequest = UUID()
        submitControl { try $0.approve(session: selected, approval: card.id, decision: decision) }
    }

    func reopen() {
        guard let selected, connected, !controlSubmitting else { return }
        submitControl { try $0.reopen(session: selected) }
    }

    func setRalph(_ on: Bool) {
        guard let selected, connected, !controlSubmitting else { return }
        controlSubmitting = true
        transport.submit({ try $0.setRalph(session: selected, on: on) }) { [weak self] result in
            guard let self else { return }
            controlSubmitting = false
            switch result {
            case .success(let operation):
                desiredRalph = on
                ralphRequest = operation
                accept(operation)
            case .failure(let failure): error = failure.localizedDescription
            }
        }
    }

    private func submitControl(_ call: @escaping @Sendable (PhoneClient) throws -> PhoneSubmission) {
        controlSubmitting = true
        transport.submit(call) { [weak self] result in
            guard let self else { return }
            controlSubmitting = false
            switch result {
            case .success(let operation): accept(operation)
            case .failure(let failure): error = failure.localizedDescription
            }
        }
    }

    func loadRows(_ ids: [String]) {
        guard let selected else { return }
        for offset in stride(from: 0, to: ids.count, by: 64) {
            let page = Array(ids[offset..<min(offset + 64, ids.count)])
            transport.read({ try $0.entries(session: selected, entries: page) }) { [weak self] result in
                guard let self, self.selected == selected else { return }
                switch result {
                case .success(let entries): for entry in entries { rows[entry.id] = entry }
                case .failure(let failure): error = failure.localizedDescription
                }
            }
        }
    }

    private func changedLink(_ state: PhoneLinkState) {
        link = state
        if case .connected = state {} else { for source in sources.values { source.suspend() } }
        if case .connected = state {} else if var cached = transcript {
            cached.currentSnapshot = false
            cached.ready = false
            transcript = cached
        }
        if case .connected = state, !catalogRequested {
            catalogRequested = true
            transport.command({ try $0.watchSessions() }) { [weak self] result in
                if case .failure(let failure) = result {
                    self?.catalogRequested = false
                    self?.error = failure.localizedDescription
                }
            }
        }
        if case .disconnected(let message) = state, let message { error = message }
    }

    private func received(_ batch: PhoneBatch) {
        for failure in batch.failures { error = failure.message }
        for body in batch.bodies where body.session() == selected && body.isSource() {
            sources[body.entry()]?.refresh(body)
        }
        guard batch.catalogChanged || !batch.operations.isEmpty || !batch.sessions.isEmpty else { return }
        let selected = selected
        let known = Set(orderedEntries)
        let cached = Set(rows.keys)
        transport.read({ client in
            try PhoneHostUpdate.read(client: client, batch: batch, selected: selected, knownIDs: known, cachedIDs: cached)
        }) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let update):
                if let catalog = update.catalog { self.catalog = catalog }
                if self.selected == selected {
                    if let approvals = update.approvals { self.approvals = approvals }
                    if let transcript = update.transcript {
                        self.transcript = transcript
                        if transcript.currentSnapshot { for source in sources.values { source.resume() } }
                    }
                    if let order = update.order {
                        orderedEntries = order
                        let retained = Set(order)
                        rows = rows.filter { retained.contains($0.key) }
                    }
                    for entry in update.rows { rows[entry.id] = entry }
                }
                for operation in update.operations {
                    accept(operation)
                }
                if update.order != nil || !update.rows.isEmpty || update.approvals != nil || !update.operations.isEmpty {
                    contentChanged()
                }
            case .failure(let failure): error = failure.localizedDescription
            }
        }
    }

    private func accept(_ operation: PhoneOperation) {
        if operations[operation.key] == nil { operationOrder.append(operation.key) }
        operations[operation.key] = operation
        if ralphRequest?.key == operation.key { ralphRequest = operation }
        completeCreation(operation)
        switch operation.state {
        case .accepted where !operation.needsEcho:
            operations.removeValue(forKey: operation.key)
            operationOrder.removeAll { $0 == operation.key }
            let key = operation.key
            transport.read({ try $0.forgetOperation(operation: key) }, completion: report)
        case .rejected(let message), .uncertain(let message): error = message
        default: break
        }
    }

    private func completeCreation(_ operation: PhoneOperation) {
        guard creation == operation.key else { return }
        switch operation.state {
        case .accepted:
            if let session = operation.session { select(session) }
            creation = nil
            creating = false
        case .rejected(let message), .uncertain(let message):
            error = message
            creating = false
        case .pending, .queued: break
        }
    }

    private func report(_ result: Result<Void, any Error>) {
        if case .failure(let failure) = result { error = failure.localizedDescription }
    }
}
