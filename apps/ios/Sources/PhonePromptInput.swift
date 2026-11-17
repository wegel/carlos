// Keep native editing, selection and software Return while giving hardware Return an explicit Send action.
import SwiftUI
import UIKit

struct PhonePromptInput: UIViewRepresentable {
    @Binding var text: String
    let send: () -> Void

    func makeUIView(context: Context) -> PhonePromptTextView {
        let view = PhonePromptTextView()
        view.backgroundColor = .clear
        view.textColor = UIColor(PhoneTheme.text)
        view.tintColor = UIColor(PhoneTheme.blue)
        view.textContainerInset = UIEdgeInsets(top: 12, left: 14, bottom: 12, right: 14)
        view.textContainer.lineFragmentPadding = 0
        view.keyboardDismissMode = .interactive
        view.autocorrectionType = .no
        view.autocapitalizationType = .none
        view.spellCheckingType = .no
        view.smartQuotesType = .no
        view.smartDashesType = .no
        view.font = Self.textAttributes[.font] as? UIFont
        view.typingAttributes = Self.textAttributes
        view.accessibilityLabel = "Prompt"
        view.accessibilityIdentifier = "composer.draft"
        view.delegate = context.coordinator
        view.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        return view
    }

    func updateUIView(_ view: PhonePromptTextView, context: Context) {
        context.coordinator.parent = self
        view.send = send
        if view.isEditable != context.environment.isEnabled { view.isEditable = context.environment.isEnabled }
        if view.text != text, view.markedTextRange == nil {
            let attributes = Self.textAttributes
            view.attributedText = NSAttributedString(string: text, attributes: attributes)
            view.typingAttributes = attributes
            view.selectedRange = NSRange(location: (text as NSString).length, length: 0)
        }
    }

    private static var textAttributes: [NSAttributedString.Key: Any] {
        let paragraph = NSMutableParagraphStyle()
        paragraph.minimumLineHeight = 22.5
        paragraph.maximumLineHeight = 22.5
        return [
            .font: UIFont(name: "InterVariable", size: 15) ?? UIFont.systemFont(ofSize: 15),
            .foregroundColor: UIColor(PhoneTheme.text), .paragraphStyle: paragraph
        ]
    }

    func sizeThatFits(_ proposal: ProposedViewSize, uiView: PhonePromptTextView, context: Context) -> CGSize? {
        guard let width = proposal.width, width > 0 else { return nil }
        let height = uiView.sizeThatFits(CGSize(width: width, height: .greatestFiniteMagnitude)).height
        return CGSize(width: width, height: min(5 * 22.5 + 24, max(44, height)))
    }

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    final class Coordinator: NSObject, UITextViewDelegate {
        var parent: PhonePromptInput
        init(_ parent: PhonePromptInput) { self.parent = parent }

        func textViewDidChange(_ textView: UITextView) {
            parent.text = textView.text
            textView.invalidateIntrinsicContentSize()
        }
    }
}

final class PhonePromptTextView: UITextView {
    var send: (() -> Void)?
    private var handledReturns = Set<UIPress>()

    override func pressesBegan(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        for press in presses {
            guard let key = press.key,
                  key.keyCode == .keyboardReturnOrEnter || key.keyCode == .keypadEnter else { continue }
            if handleReturn(modifiers: key.modifierFlags) { handledReturns.insert(press) }
        }
        let remaining = presses.subtracting(handledReturns)
        if !remaining.isEmpty { super.pressesBegan(remaining, with: event) }
    }

    override func pressesEnded(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining = unhandledReleases(presses)
        if !remaining.isEmpty { super.pressesEnded(remaining, with: event) }
    }

    override func pressesCancelled(_ presses: Set<UIPress>, with event: UIPressesEvent?) {
        let remaining = unhandledReleases(presses)
        if !remaining.isEmpty { super.pressesCancelled(remaining, with: event) }
    }

    // Software Return keeps UIKit's normal newline path; hardware Return has an explicit action.
    func handleReturn(modifiers: UIKeyModifierFlags) -> Bool {
        guard isEditable, markedTextRange == nil,
              modifiers.intersection([.command, .control]).isEmpty else { return false }
        if modifiers.intersection([.shift, .alternate]).isEmpty { send?() }
        else {
            insertText("\n")
            delegate?.textViewDidChange?(self)
        }
        return true
    }

    private func unhandledReleases(_ presses: Set<UIPress>) -> Set<UIPress> {
        let remaining = presses.subtracting(handledReturns)
        handledReturns.subtract(presses)
        return remaining
    }
}
