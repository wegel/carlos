// Keep each host's device-local draft and visible facts outside the shared Rust session.
import Foundation
import Observation

@MainActor @Observable
final class PhoneHostModel: Identifiable {
    nonisolated let id: String
    var configuration: PhoneHost
    var link: PhoneLinkState = .disconnected(nil)
    var catalog: [PhoneSession] = []
    var selected: String? {
        didSet { defaults.set(selected, forKey: "carlos.hosts.\(id).session") }
    }
    var transcript: PhoneTranscript?
    var orderedEntries: [String] = []
    var rows: [String: PhoneEntry] = [:]
    var operations: [String: PhoneOperation] = [:]
    var operationOrder: [String] = []
    var echoAnchors: [String: PhoneEchoAnchor] = [:]
    var approvals: [PhoneApproval] = []
    var ralphRequest: PhoneOperation?
    var desiredRalph: Bool?
    var contentVersion = UUID()
    var followRequest = UUID()
    var controlSubmitting = false
    let history = PhoneHistoryModel()
    var approvalRequests: [String: ApprovalSubmission] = [:]
    private var draftText = ""
    private(set) var draftVersion: UInt64 = 0
    var draft: String {
        get { draftText }
        set { draftVersion += 1; draftText = newValue }
    }
    var submitting = false
    @ObservationIgnored var reopening: String?
    var creating = false
    var error: String?
    @ObservationIgnored let transport = PhoneTransport()
    @ObservationIgnored private(set) var connectionIdentity: UUID?
    @ObservationIgnored private var catalogRequested = false
    @ObservationIgnored private var creation: String?
    @ObservationIgnored private let bodyBudget: PhoneBodyBudget
    @ObservationIgnored private let defaults: UserDefaults
    @ObservationIgnored private var sources: [String: PhoneSourceModel] = [:]
    @ObservationIgnored private var outputs: [String: PhoneOutputModel] = [:]

    init(configuration: PhoneHost, bodyBudget: PhoneBodyBudget = PhoneBodyBudget(), defaults: UserDefaults = .standard) {
        id = configuration.id
        self.configuration = configuration
        self.bodyBudget = bodyBudget
        self.defaults = defaults
    }

    func contentChanged() { contentVersion = UUID() }

    // Native keystrokes update the model without asking UIKit to replace its own text.
    func editDraft(_ text: String) { draftText = text }

    func loadedSource(for entry: String) -> PhoneSourceModel? { sources[entry] }

    func output(for entry: String) -> PhoneOutputModel? {
        guard let selected else { return nil }
        if let output = outputs[entry] { return output }
        let output = PhoneOutputModel(session: selected, entry: entry, transport: transport)
        if !connected || transcript?.currentSnapshot != true { output.suspend() }
        outputs[entry] = output
        return output
    }

    func closeOutput(_ output: PhoneOutputModel) {
        output.close()
        if outputs[output.entry] === output { outputs.removeValue(forKey: output.entry) }
    }

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
        do { _ = connect(url: try configuration.url(), token: try HostTokens.read(for: id)) }
        catch {
            connectionIdentity = nil
            transport.disconnect()
            changedLink(.disconnected(PhoneFailureMessage.describe(error)))
        }
    }

    func connect(url: URL, token: String) -> UUID {
        let identity = UUID()
        connectionIdentity = identity
        let callbacks = PhoneCallbacks(
            received: { [weak self] batch in
                guard let self, self.connectionIdentity == identity else { return }
                self.received(batch)
            },
            link: { [weak self] state in
                guard let self, self.connectionIdentity == identity else { return }
                self.changedLink(state)
            }
        )
        error = nil
        changedLink(.connecting)
        transport.connect(url: url, token: token, callbacks: callbacks)
        return identity
    }

    func cancelConnection(_ identity: UUID) {
        guard connectionIdentity == identity else { return }
        connectionIdentity = nil
        transport.disconnect()
        changedLink(.disconnected(nil))
    }

    func select(_ session: String) {
        guard connected else { error = "Reconnect to choose a session"; return }
        guard selected != session else { return }
        history.reset()
        draft = ""
        let old = selected
        selected = session
        for source in sources.values { source.close() }
        sources = [:]
        for output in outputs.values { output.close() }
        outputs = [:]
        transcript = nil
        orderedEntries = []
        rows = [:]
        approvals = []
        approvalRequests = [:]
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
                error = PhoneFailureMessage.describe(failure)
            }
        }
    }

    func send() {
        guard let selected, !submitting, connected, transcript?.ready == true,
              !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return }
        let text = draft
        let echoAnchor = PhoneEchoAnchor(entry: orderedEntries.last, queued: transcript?.info.activity.canInterrupt == true)
        submitting = true
        transport.submit({ try $0.sendPrompt(session: selected, text: text) }) { [weak self] result in
            guard let self else { return }
            submitting = false
            switch result {
            case .success(let operation):
                echoAnchors[operation.key] = echoAnchor
                accept(operation)
                if self.selected == selected {
                    followRequest = UUID()
                    if draft == text { draft = "" }
                }
            case .failure(let failure): error = PhoneFailureMessage.describe(failure)
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
                case .failure(let failure): error = PhoneFailureMessage.describe(failure)
                }
            }
        }
    }

    private func changedLink(_ state: PhoneLinkState) {
        link = state
        if case .connected = state {} else { history.reset() }
        if case .connected = state {} else { for source in sources.values { source.suspend() } }
        if case .connected = state {} else { for output in outputs.values { output.suspend() } }
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
                    self?.error = PhoneFailureMessage.describe(failure)
                }
            }
        }
        if case .connected = state, selected == nil,
           let saved = defaults.string(forKey: "carlos.hosts.\(id).session") {
            select(saved)
        }
        if case .disconnected(let message) = state, let message { error = message }
    }

    private func received(_ batch: PhoneBatch) {
        let historyFailure = history.receive(batch)
        for failure in batch.failures where historyFailure == nil || failure.requestId != historyFailure { error = failure.message }
        for body in batch.bodies where body.session() == selected {
            if body.isSource() { sources[body.entry()]?.refresh(body) }
            else { outputs[body.entry()]?.refresh(body) }
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
                    if let approvals = update.approvals {
                        self.approvals = approvals
                        let live = Set(approvals.map(\.id))
                        approvalRequests = approvalRequests.filter { live.contains($0.key) }
                    }
                    if let transcript = update.transcript {
                        self.transcript = transcript
                        if transcript.currentSnapshot { for source in sources.values { source.resume() } }
                        if transcript.currentSnapshot { for output in outputs.values { output.resume() } }
                    }
                    if let order = update.order {
                        orderedEntries = order
                        let retained = Set(order)
                        rows = rows.filter { retained.contains($0.key) }
                        retainBodies(retained)
                    }
                    for entry in update.rows { rows[entry.id] = entry }
                    if let transcript = update.transcript { history.updated(transcript, first: orderedEntries.first) }
                }
                for operation in update.operations {
                    accept(operation)
                }
                if update.order != nil || !update.rows.isEmpty || update.approvals != nil || !update.operations.isEmpty {
                    contentChanged()
                }
            case .failure(let failure): error = PhoneFailureMessage.describe(failure)
            }
        }
    }

    func accept(_ operation: PhoneOperation) {
        if operations[operation.key] == nil { operationOrder.append(operation.key) }
        operations[operation.key] = operation
        if ralphRequest?.key == operation.key { ralphRequest = operation }
        completeCreation(operation)
        completeReopen(operation)
        switch operation.state {
        case .accepted where !operation.needsEcho:
            echoAnchors.removeValue(forKey: operation.key)
            operations.removeValue(forKey: operation.key)
            operationOrder.removeAll { $0 == operation.key }
            let key = operation.key
            transport.read({ try $0.forgetOperation(operation: key) }, completion: report)
        case .rejected(let message), .uncertain(let message): error = message
        default: break
        }
    }

    private func retainBodies(_ ids: Set<String>) {
        for (id, source) in sources where !ids.contains(id) { source.close() }
        sources = sources.filter { ids.contains($0.key) }
        for (id, output) in outputs where !ids.contains(id) { output.close() }
        outputs = outputs.filter { ids.contains($0.key) }
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
        if case .failure(let failure) = result { error = PhoneFailureMessage.describe(failure) }
    }
}
