// Lay out semantic Rust blocks with native fonts; this view never parses Markdown punctuation.
import SwiftUI

struct PhoneMarkdownView: View {
    let document: PhoneDocument
    let color: Color
    var role: PhoneEntryKind = .answer
    var changed: @MainActor () -> Void = {}

    var body: some View {
        LazyVStack(alignment: .leading, spacing: PhoneTheme.paragraphGap) {
            ForEach(0..<Int(document.blockCount()), id: \.self) { index in
                PhoneMarkdownBlockView(document: document, index: UInt32(index), color: color, role: role, changed: changed)
            }
        }
    }
}

struct PhoneMarkdownBlockView: View {
    let document: PhoneDocument
    let index: UInt32
    let color: Color
    var role: PhoneEntryKind = .answer
    var changed: @MainActor () -> Void = {}
    @State private var block: PhoneMarkdownBlock?
    @State private var error: String?

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
            let document = document, index = index
            let result = await Task.detached(priority: .userInitiated) { Result { try document.block(index: index) } }.value
            guard !Task.isCancelled else { return }
            switch result { case .success(let value): block = value; case .failure(let failure): error = failure.localizedDescription }
            changed()
        }
    }

    @ViewBuilder private func content(_ block: PhoneMarkdownBlock) -> some View {
        switch block.kind {
        case .paragraph:
            Text(styled(block.spans, size: role == .notice ? 11.5 : 15))
                .lineSpacing(role == .notice ? 0 : PhoneTheme.proseLeading).textSelection(.enabled)
        case .heading(let level):
            Text(styled(block.spans, size: max(15, 25 - CGFloat(level) * 2), strong: true)).textSelection(.enabled)
        case .code(let language):
            VStack(alignment: .leading, spacing: 6) {
                if let language { Text(language).font(PhoneTheme.mono(11)).foregroundStyle(PhoneTheme.overlay2) }
                ScrollView(.horizontal) {
                    Text(block.spans.map(\.text).joined()).font(PhoneTheme.mono(12.5)).fixedSize(horizontal: true, vertical: false)
                        .textSelection(.enabled).foregroundStyle(PhoneTheme.text)
                }.scrollIndicators(.hidden)
            }.padding(10).background(PhoneTheme.crust).clipShape(RoundedRectangle(cornerRadius: 10))
        case .rule:
            Rectangle().fill(PhoneTheme.surface0).frame(height: 1).padding(.vertical, 6)
        case .tableRow(let header, let columns):
            ScrollView(.horizontal) {
                HStack(alignment: .top, spacing: 16) {
                    ForEach(block.cells.indices, id: \.self) { column in
                        Text(styled(block.cells[column], strong: header)).textSelection(.enabled)
                            .frame(width: 140, alignment: alignment(columns.indices.contains(column) ? columns[column] : .unspecified))
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
