// Retain original bytes and format one complete observed Markdown document at a time off the socket queue.
import Foundation
import Observation

@MainActor @Observable
final class PhoneSourceModel {
    let session: String
    let entry: String
    let markup: PhoneMarkdownModel
    var document: PhoneRenderedDocument? { markup.document }
    var acceptedBytes: UInt64 = 0
    var totalBytes: UInt64?
    var done = false
    private var failure: String?
    var error: String? { failure ?? markup.error }
    @ObservationIgnored private(set) var file: PhoneSourceFile?
    @ObservationIgnored private var body: PhoneBody?
    @ObservationIgnored private let transport: PhoneTransport
    @ObservationIgnored private let budget: PhoneBodyBudget
    @ObservationIgnored private var active = true
    @ObservationIgnored private var reading = false
    @ObservationIgnored private var watching = false
    @ObservationIgnored private var revision: UInt64?
    @ObservationIgnored private var suspended = false
    @ObservationIgnored private var restoring = false
    @ObservationIgnored private var paused = false
    @ObservationIgnored private var opening = false
    @ObservationIgnored private var viewers: Set<UUID> = []

    init(session: String, entry: String, transport: PhoneTransport, budget: PhoneBodyBudget, markdown: Bool, changed: @escaping @MainActor @Sendable () -> Void) {
        markup = PhoneMarkdownModel(markdown: markdown, changed: changed)
        self.session = session
        self.entry = entry
        self.transport = transport
        self.budget = budget
        open()
    }

    func refresh(_ current: PhoneBody) {
        guard active, !paused, current.session() == session, current.entry() == entry, current.isSource() else { return }
        guard let body else { return }
        if !current.sameRead(other: body) {
            replace(current)
            return
        }
        refresh()
    }

    func activate(viewer: UUID, revision: UInt64?, bytes: UInt64?, complete: Bool) {
        guard active else { return }
        viewers.insert(viewer)
        paused = false
        if complete, done, self.revision == revision, acceptedBytes == bytes, file != nil { return }
        if body == nil { open() }
    }

    func pause(viewer: UUID) {
        viewers.remove(viewer)
        guard viewers.isEmpty else { return }
        paused = true
        reading = false
        watching = false
        if let body { transport.batch({ try $0.closeBody(body: body) }) { _ in } }
        body = nil
        // Retain the bounded original file and parsed document so a lazy row keeps its height.
    }

    func suspend() { suspended = true; reading = false }

    func resume() {
        guard active, !paused, suspended else { return }
        suspended = false
        failure = nil
        restoring = true
        refresh()
    }

    func close() {
        active = false
        viewers.removeAll()
        if let body { transport.batch({ try $0.closeBody(body: body) }) { _ in } }
        body = nil
        file = nil
        markup.clear()
    }

    private func open() {
        guard active, !paused, !opening else { return }
        opening = true
        let session = session, entry = entry, budget = budget, retained = file
        transport.read({ client in
            let file = try retained ?? PhoneSourceFile(budget: budget)
            let body = try client.openSource(session: session, entry: entry, document: false)
            return SourceOpened(body: body, file: file)
        }) { [weak self, transport] result in
            self?.opening = false
            switch result {
            case .success(let opened):
                guard let self, active, !paused else { transport.batch({ try $0.closeBody(body: opened.body) }) { _ in }; return }
                body = opened.body
                file = opened.file
                watching = false
                readNext()
            case .failure(let failure): self?.failure = PhoneFailureMessage.describe(failure)
            }
        }
    }

    private func replace(_ current: PhoneBody) {
        body = current
        reading = false
        done = false
        // Keep the accepted prefix and old document while Rust re-establishes a cursor.
        // Only a checked different body revision may replace those original bytes.
        refresh()
    }

    private func refresh() {
        guard active, !paused, let body, let file else { return }
        reading = false
        let revision = revision, budget = budget
        transport.read({ client in
            let window = try client.sourceWindow(body: body)
            let replaced = revision != nil && window.revision != nil && revision != window.revision
            let target = try replaced ? PhoneSourceFile(budget: budget) : file
            let offset = target.byteCount
            if let total = window.totalBytes, offset > total {
                throw PhoneError.InvalidMessage(message: "Original source shrank without a new body revision; accepted text is retained")
            }
            if offset <= window.end {
                let text = try client.sourceText(body: body, offset: offset, limit: 65536)
                try target.append(text, at: offset)
            }
            return SourceProgress(bytes: target.byteCount, window: window, file: target, replaced: replaced)
        }) { [weak self] result in
            guard let self, active, !paused, self.body?.sameRead(other: body) == true, self.file === file else { return }
            switch result {
            case .success(let progress):
                restoring = false
                if progress.replaced {
                    self.file = progress.file
                    markup.clear()
                }
                if let revision = progress.window.revision { self.revision = revision }
                acceptedBytes = progress.bytes
                totalBytes = progress.window.totalBytes
                done = progress.window.done && !progress.window.hasMore && progress.bytes == progress.window.end
                if progress.bytes < progress.window.end { refresh() }
                else if progress.window.hasMore { if !watching { readNext() } }
                else {
                    markup.setSource(progress.file, bytes: progress.bytes)
                    if !progress.window.done { watch() }
                }
            case .failure(let failure):
                if restoring, case PhoneError.StaleRead = failure {
                    restoring = false
                    open()
                } else { self.failure = PhoneFailureMessage.describe(failure) }
            }
        }
    }

    private func readNext() {
        guard active, !paused, !reading, let body else { return }
        reading = true
        transport.command({ try $0.readBody(body: body) }) { [weak self] result in
            if case .failure(let failure) = result { self?.reading = false; self?.failure = PhoneFailureMessage.describe(failure) }
        }
    }

    private func watch() {
        guard active, !paused, !watching, let body else { return }
        watching = true
        transport.command({ try $0.watchBody(body: body) }) { [weak self] result in
            if case .failure(let failure) = result { self?.watching = false; self?.failure = PhoneFailureMessage.describe(failure) }
        }
    }

}

private struct SourceOpened: Sendable {
    let body: PhoneBody
    let file: PhoneSourceFile
}

private struct SourceProgress: Sendable {
    let bytes: UInt64
    let window: PhoneSourceWindow
    let file: PhoneSourceFile
    let replaced: Bool
}
