// Paint classified normalized pieces without interpreting terminal controls or changing their text.
import SwiftUI

struct PhoneOutputView: View {
    let model: PhoneOutputModel
    @State private var text = AttributedString()

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if model.window?.missingPrefix == true {
                Text("Showing the latest output; earlier lines were omitted.")
                    .font(PhoneTheme.mono(11)).foregroundStyle(PhoneTheme.overlay2)
            }
            if model.window == nil, model.error == nil {
                ProgressView("Loading tool output").font(PhoneTheme.mono(11.5))
            } else {
                ScrollView(.horizontal) {
                    Text(text).font(PhoneTheme.mono(11.5)).lineSpacing(PhoneTheme.outputLeading)
                        .fixedSize(horizontal: true, vertical: false).textSelection(.enabled)
                }.scrollIndicators(.hidden)
            }
            if let error = model.error {
                Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red)
                Button("Retry") { model.retry() }.frame(minHeight: 44)
            }
        }
        .padding(.vertical, 8).padding(.horizontal, 10).frame(maxWidth: .infinity, alignment: .leading)
        .background(PhoneTheme.crust).clipShape(RoundedRectangle(cornerRadius: 10))
        .overlay { RoundedRectangle(cornerRadius: 10).stroke(PhoneTheme.surface0, lineWidth: 1) }
        .accessibilityIdentifier("tool.output.\(model.entry)")
        .task(id: model.version) {
            let pieces = model.window?.pieces ?? []
            let formatted = await Task.detached(priority: .userInitiated) { PhoneOutputText.format(pieces) }.value
            guard !Task.isCancelled else { return }
            text = formatted
        }
    }
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
