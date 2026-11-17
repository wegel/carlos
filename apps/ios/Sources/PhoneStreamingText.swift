// Paint the live caret after native text layout; it never becomes selected or copied source text.
import SwiftUI

struct PhoneStreamingText: View {
    let text: Text
    let streaming: Bool
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        if streaming {
            Group {
                if reduceMotion { text.textRenderer(AnswerCaret(visible: true)) }
                else {
                    TimelineView(.periodic(from: .now, by: 0.55)) { clock in
                        text.textRenderer(AnswerCaret(visible: Int(clock.date.timeIntervalSinceReferenceDate / 0.55) % 2 == 0))

                    }
                }
            // Selectable text bypasses TextRenderer on the checked iOS build.
            }.textSelection(.disabled).accessibilityIdentifier("answer.streaming")
        } else { text.textSelection(.enabled) }
    }
}

private struct AnswerCaret: TextRenderer {
    let visible: Bool
    var displayPadding: EdgeInsets { EdgeInsets(top: 0, leading: 0, bottom: 0, trailing: 8) }

    func draw(layout: Text.Layout, in context: inout GraphicsContext) {
        for line in layout { context.draw(line) }
        guard let line = layout.last else { return }
        let bounds = line.typographicBounds.rect
        guard visible else { return }
        let height = min(18, bounds.height)
        let caret = CGRect(x: bounds.maxX + 2, y: bounds.minY + (bounds.height - height) / 2, width: 6, height: height)
        context.fill(Path(caret), with: .color(PhoneTheme.green))
    }
}

