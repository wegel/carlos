// Retain parsed block facts across cell reuse; keep grammar work off the UI thread.
import SwiftUI

@MainActor
final class PhoneRenderedDocument {
    private let source: PhoneDocument
    private var blocks: [UInt32: PhoneRenderedBlock] = [:]

    init(source: PhoneDocument) { self.source = source }
    func blockCount() -> UInt64 { source.blockCount() }

    func block(index: UInt32) -> PhoneRenderedBlock {
        if let block = blocks[index] { return block }
        let block = PhoneRenderedBlock(source: source, index: index)
        blocks[index] = block
        return block
    }
}

@MainActor
final class PhoneRenderedBlock: ObservableObject {
    let block: PhoneMarkdownBlock?
    @Published private(set) var code: [PhoneOutputChunk]?
    @Published private(set) var error: String?
    private let source: PhoneDocument
    private let index: UInt32
    private var rendering: Task<Result<[PhoneOutputChunk], any Error>, Never>?

    init(source: PhoneDocument, index: UInt32) {
        self.source = source
        self.index = index
        do {
            var value = try source.block(index: index)
            if case .code = value.kind { value.spans = [] }
            block = value
        } catch {
            block = nil
            self.error = PhoneFailureMessage.describe(error)
        }
    }

    func loadCode() async -> Bool {
        guard let block, case .code = block.kind, code == nil, error == nil else { return false }
        if rendering == nil {
            let source = source, index = index
            rendering = Task.detached(priority: .userInitiated) {
                Result { PhoneOutputChunk.split(PhoneFileOutput(text: PhoneCodeText.format(try source.code(index: index)))) }
            }
        }
        guard let rendering else { return false }
        let result = await rendering.value
        guard code == nil, error == nil else { return false }
        self.rendering = nil
        switch result {
        case .success(let chunks): code = chunks
        case .failure(let failure): error = PhoneFailureMessage.describe(failure)
        }
        return true
    }
}
