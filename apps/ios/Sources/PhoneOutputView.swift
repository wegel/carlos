// Paint classified normalized pieces without interpreting terminal controls or changing their text.
import SwiftUI

struct PhoneOutputView: View {
    let model: PhoneOutputModel
    var filePath: String?
    @State private var chunks: [PhoneOutputChunk] = []
    @State private var numberWidth: CGFloat = 24
    @State private var contentHeight: CGFloat = 22

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if model.window?.missingPrefix == true {
                Text("Showing the latest output; earlier lines were omitted.")
                    .font(PhoneTheme.mono(11)).foregroundStyle(PhoneTheme.overlay2)
            }
            if model.window == nil, model.error == nil {
                ProgressView("Loading tool output").font(PhoneTheme.mono(11.5))
            } else {
                ScrollView([.horizontal, .vertical]) {
                    LazyVStack(alignment: .leading, spacing: PhoneTheme.outputLeading) {
                        ForEach(chunks) { chunk in
                            HStack(alignment: .top, spacing: 12) {
                                if let numbers = chunk.numbers {
                                    Text(numbers).foregroundStyle(PhoneTheme.overlay1)
                                        .frame(width: numberWidth, alignment: .trailing).accessibilityHidden(true)
                                }
                                Text(chunk.text).textSelection(.enabled)
                            }.font(PhoneTheme.mono(11.5)).lineSpacing(PhoneTheme.outputLeading)
                                .fixedSize(horizontal: true, vertical: true)
                        }
                    }.onGeometryChange(for: CGFloat.self) { $0.size.height } action: { contentHeight = $0 }
                }.frame(height: min(320, contentHeight)).scrollIndicators(.visible)
                    .defaultScrollAnchor(.topLeading, for: .alignment)
            }
            if let error = model.error {
                Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red)
                Button("Retry") { model.retry() }.frame(minHeight: 44)
            }
        }
        .padding(.vertical, 8).padding(.horizontal, 10).frame(maxWidth: .infinity, alignment: .leading)
        .background(PhoneTheme.crust).clipShape(RoundedRectangle(cornerRadius: 10))
        .overlay { RoundedRectangle(cornerRadius: 10).stroke(PhoneTheme.surface0, lineWidth: 1) }
        .accessibilityElement(children: .contain).accessibilityIdentifier("tool.output.\(model.entry)")
        .task(id: OutputFormatKey(version: model.version, path: filePath)) {
            let pieces = model.window?.pieces ?? []
            let path = filePath
            let formatted = await Task.detached(priority: .userInitiated) {
                let output = path.map { PhoneFileOutput.format(pieces, path: $0) }
                    ?? PhoneFileOutput(text: PhoneOutputText.format(pieces))
                let digits = output.numbers.split(separator: "\n").map(\.count).max() ?? 0
                return (PhoneOutputChunk.split(output), digits)
            }.value
            guard !Task.isCancelled else { return }
            chunks = formatted.0
            numberWidth = max(24, CGFloat(formatted.1) * 6.9)
        }
    }
}

private struct OutputFormatKey: Hashable {
    let version: UUID
    let path: String?
}

enum PhoneOutputText {
    static func format(_ pieces: [PhoneOutputPiece]) -> AttributedString {
        var result = AttributedString()
        for piece in pieces {
            var span = AttributedString(piece.text)
            span.foregroundColor = color(piece.kind)
            if piece.kind == .addition { span.backgroundColor = PhoneTheme.green.opacity(0.08) }
            if piece.kind == .deletion { span.backgroundColor = PhoneTheme.red.opacity(0.08) }
            result.append(span)
        }
        return result
    }

    private static func color(_ kind: PhoneOutputKind) -> Color {
        switch kind {
        case .plain: PhoneTheme.subtext1
        case .command: PhoneTheme.overlay2
        case .success, .addition: PhoneTheme.green
        case .error, .deletion: PhoneTheme.red
        case .hunk: PhoneTheme.mauve
        case .context: PhoneTheme.subtext0
        }
    }
}
