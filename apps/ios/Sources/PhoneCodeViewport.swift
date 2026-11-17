// Draw only nearby text blocks in a bounded native viewport, with optional original file numbers.
import SwiftUI

struct PhoneCodeViewport: View {
    let chunks: [PhoneOutputChunk]
    var fontSize: CGFloat = 11.5
    var lineSpacing: CGFloat = PhoneTheme.outputLeading
    var numberWidth: CGFloat = 24
    var streaming = false
    var textIdentifier = ""
    var scrollIdentifier = ""
    @State private var contentHeight: CGFloat = 22

    var body: some View {
        GeometryReader { viewport in
            ScrollView([.horizontal, .vertical]) {
                LazyVStack(alignment: .leading, spacing: lineSpacing) {
                    ForEach(chunks) { chunk in
                        HStack(alignment: .top, spacing: 12) {
                            if let numbers = chunk.numbers {
                                Text(numbers).foregroundStyle(PhoneTheme.overlay1)
                                    .frame(width: numberWidth, alignment: .trailing).accessibilityHidden(true)
                            }
                            PhoneStreamingText(text: Text(chunk.text), streaming: streaming && chunk.id == chunks.last?.id)
                                .accessibilityIdentifier(streaming && chunk.id == chunks.last?.id ? "answer.streaming" : textIdentifier)
                        }.font(PhoneTheme.mono(fontSize)).lineSpacing(lineSpacing)
                            .fixedSize(horizontal: true, vertical: true)
                    }
                }.frame(minWidth: viewport.size.width, alignment: .leading)
                    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { contentHeight = $0 }
            }.scrollIndicators(.visible)
                .defaultScrollAnchor(.topLeading, for: .alignment)
                .accessibilityIdentifier(scrollIdentifier)
        }.frame(height: min(320, contentHeight))
    }
}
