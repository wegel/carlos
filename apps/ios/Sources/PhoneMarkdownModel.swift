// Keep one formatter per visible document; new text replaces the pending target, never starts parallel parses.
import Foundation
import Observation

@MainActor @Observable
final class PhoneMarkdownModel {
    private let markdown: Bool
    private let changed: @MainActor @Sendable () -> Void
    var document: PhoneDocument?
    var error: String?
    @ObservationIgnored private var target: PhoneCopySource?
    @ObservationIgnored private var identity = UUID()
    @ObservationIgnored private var formatting = false
    @ObservationIgnored private var inline: String?
    @ObservationIgnored private weak var file: PhoneSourceFile?
    @ObservationIgnored private var bytes: UInt64?

    init(markdown: Bool = true, changed: @escaping @MainActor @Sendable () -> Void = {}) {
        self.markdown = markdown
        self.changed = changed
    }

    func setInline(_ text: String) {
        guard inline != text else { return }
        inline = text
        file = nil
        bytes = nil
        update(.inline(text))
    }

    func setSource(_ file: PhoneSourceFile, bytes: UInt64) {
        guard self.file !== file || self.bytes != bytes else { return }
        inline = nil
        self.file = file
        self.bytes = bytes
        update(.original(file, bytes))
    }

    func clear() {
        target = nil
        file = nil
        inline = nil
        bytes = nil
        identity = UUID()
        document = nil
        error = nil
    }

    private func update(_ target: PhoneCopySource) {
        self.target = target
        identity = UUID()
        error = nil
        formatNext()
    }

    private func formatNext() {
        guard !formatting, let target else { return }
        formatting = true
        let identity = identity
        let markdown = markdown
        Task { [weak self] in
            let result = await Task.detached(priority: .userInitiated) {
                Result {
                    let source = try target.text()
                    return try markdown ? PhoneDocument(source: source) : PhoneDocument.plain(source: source)
                }
            }.value
            guard let self else { return }
            formatting = false
            if self.identity == identity {
                switch result {
                case .success(let document): self.document = document
                case .failure(let failure): error = PhoneFailureMessage.describe(failure)
                }
                changed()
            } else { formatNext() }
        }
    }
}
