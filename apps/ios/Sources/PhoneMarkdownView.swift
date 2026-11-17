// Lay out semantic Rust blocks with native fonts; this view never parses Markdown punctuation.
import SwiftUI

struct PhoneMarkdownView: View {
    let document: PhoneRenderedDocument
    let color: Color
    var role: PhoneEntryKind = .answer
    var streaming = false
    var changed: @MainActor () -> Void = {}

    var body: some View {
        // Long answers already use independent rows in PhoneTranscriptItem. Nesting
        // another lazy stack here makes inline row heights unstable during scrolling.
        VStack(alignment: .leading, spacing: PhoneTheme.paragraphGap) {
            ForEach(0..<Int(document.blockCount()), id: \.self) { index in
                PhoneMarkdownBlockView(document: document, index: UInt32(index), color: color, role: role, streaming: streaming && index + 1 == Int(document.blockCount()), changed: changed)
            }
        }
    }
}

struct PhoneMarkdownBlockView: View {
    let document: PhoneRenderedDocument
    let index: UInt32
    let color: Color
    var role: PhoneEntryKind = .answer
    var streaming = false
    var changed: @MainActor () -> Void = {}
    @ObservedObject private var prepared: PhoneRenderedBlock

    init(document: PhoneRenderedDocument, index: UInt32, color: Color, role: PhoneEntryKind = .answer,
         streaming: Bool = false, changed: @escaping @MainActor () -> Void = {}) {
        self.document = document
        self.index = index
        self.color = color
        self.role = role
        self.streaming = streaming
        self.changed = changed
        _prepared = ObservedObject(wrappedValue: document.block(index: index))
    }

    private var block: PhoneMarkdownBlock? { prepared.block }
    private var error: String? { prepared.error }
    private var code: [PhoneOutputChunk]? { prepared.code }

    var body: some View {
        Group {
            if let block {
                HStack(alignment: .top, spacing: 8) {
                    if block.quoteDepth > 0 { Rectangle().fill(PhoneTheme.surface1).frame(width: 2) }
                    if let marker = block.marker { Text(marker).font(PhoneTheme.prose()).frame(minWidth: 14, alignment: .trailing) }
                    content(block).frame(maxWidth: .infinity, alignment: .leading)
                }
                .padding(.leading, CGFloat(block.indent) * 14)
                .foregroundStyle(color)
            } else if let error { Text(error).foregroundStyle(PhoneTheme.red) }
            else { Color.clear.frame(height: 22) }
        }
        .task(id: ObjectIdentifier(document)) {
            if await prepared.loadCode() { changed() }
        }
    }

    @ViewBuilder private func content(_ block: PhoneMarkdownBlock) -> some View {
        switch block.kind {
        case .paragraph:
            PhoneStreamingText(text: Text(styled(block.spans, size: role == .notice ? 11.5 : 15)), streaming: streaming)
                .lineSpacing(role == .notice ? 0 : PhoneTheme.proseLeading).textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        case .heading(let level):
            PhoneStreamingText(text: Text(styled(block.spans, size: max(15, 25 - CGFloat(level) * 2), strong: true)), streaming: streaming)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
        case .code(let language):
            VStack(alignment: .leading, spacing: 6) {
                if let language { Text(language).font(PhoneTheme.mono(11)).foregroundStyle(PhoneTheme.overlay2) }
                PhoneCodeViewport(chunks: code ?? [], fontSize: 12.5, lineSpacing: 0, streaming: streaming,
                                  textIdentifier: "code.block", scrollIdentifier: "code.viewport")
            }.padding(10).background(PhoneTheme.crust).clipShape(RoundedRectangle(cornerRadius: 10))
        case .rule:
            Rectangle().fill(PhoneTheme.surface0).frame(height: 1).padding(.vertical, 6)
        case .tableRow(let header, let columns):
            ScrollView(.horizontal) {
                HStack(alignment: .top, spacing: 16) {
                    ForEach(block.cells.indices, id: \.self) { column in
                        Text(styled(block.cells[column], strong: header)).textSelection(.enabled)
                            .frame(width: 140, alignment: alignment(columns.indices.contains(column) ? columns[column] : .unspecified))
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }.padding(.vertical, 4)
            }.scrollIndicators(.hidden)
        }
    }

    private func styled(_ spans: [PhoneMarkdownSpan], size: CGFloat = 15, strong: Bool = false) -> AttributedString {
        var result = AttributedString()
        for span in spans {
            var text = AttributedString(span.text)
            var font = span.style.code || role == .notice ? PhoneTheme.mono(role == .notice ? size : size * 0.86) : PhoneTheme.prose(size)
            if role == .user || strong || span.style.strong { font = font.weight(.semibold) }
            if role == .reasoning || span.style.emphasis { font = font.italic() }
            text.font = font
            if span.style.code { text.backgroundColor = PhoneTheme.surface0 }
            if span.style.strike { text.strikethroughStyle = .single }
            if let link = span.style.link, let url = URL(string: link), ["https", "http", "mailto"].contains(url.scheme?.lowercased() ?? "") {
                text.link = url
                text.foregroundColor = PhoneTheme.blue
            }
            result.append(text)
        }
        return result
    }

    private func alignment(_ column: PhoneMarkdownColumn) -> Alignment {
        switch column { case .unspecified, .left: .leading; case .center: .center; case .right: .trailing }
    }
}
