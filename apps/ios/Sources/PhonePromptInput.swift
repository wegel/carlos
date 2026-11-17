// Keep native editing, selection and software Return while giving hardware Return an explicit Send action.
import SwiftUI
import UIKit

struct PhonePromptInput: UIViewRepresentable {
    @Binding var text: String
    let replacementVersion: UInt64
    let send: () -> Void

    func makeUIView(context: Context) -> PhonePromptTextView {
        let view = PhonePromptTextView()
        view.backgroundColor = .clear
        view.clipsToBounds = true
        view.textColor = UIColor(PhoneTheme.text)
        view.tintColor = UIColor(PhoneTheme.blue)
        view.textContainerInset = UIEdgeInsets(top: 12, left: 0, bottom: 12, right: 0)
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
        view.prepareKeyboard()
        return view
    }

    func updateUIView(_ view: PhonePromptTextView, context: Context) {
        context.coordinator.parent = self
        view.send = send
        if view.isEditable != context.environment.isEnabled { view.isEditable = context.environment.isEnabled }
        view.replaceText(text, version: replacementVersion, attributes: Self.textAttributes)
    }

    private static var textAttributes: [NSAttributedString.Key: Any] {
        let paragraph = NSMutableParagraphStyle()
        paragraph.minimumLineHeight = 22.5
        paragraph.maximumLineHeight = 22.5
        return [
            .font: UIFont(name: "JetBrainsMono-Regular", size: 14) ?? UIFont.monospacedSystemFont(ofSize: 14, weight: .regular),
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
        }
    }
}

final class PhonePromptTextView: UITextView {
    var send: (() -> Void)?
    private var handledReturns = Set<UIPress>()
    private var lastReplacement: UInt64?
    private var idleCaret: PhoneIdleCaret?

    func prepareKeyboard() {
        idleCaret = PhoneIdleCaret(input: self)
        let toolbar = UIView(frame: CGRect(x: 0, y: 0, width: 0, height: 44))
        toolbar.autoresizingMask = .flexibleWidth
        toolbar.backgroundColor = UIColor(PhoneTheme.mantle)
        let done = UIButton(type: .system)
        done.setTitle("Done", for: .normal)
        done.setTitleColor(UIColor(PhoneTheme.blue), for: .normal)
        done.titleLabel?.font = .systemFont(ofSize: 14, weight: .semibold)
        done.addTarget(self, action: #selector(hideKeyboard), for: .touchUpInside)
        done.accessibilityLabel = "Hide keyboard"
        done.accessibilityIdentifier = "composer.keyboardDone"
        done.translatesAutoresizingMaskIntoConstraints = false
        toolbar.addSubview(done)
        NSLayoutConstraint.activate([
            done.trailingAnchor.constraint(equalTo: toolbar.trailingAnchor, constant: -12),
            done.topAnchor.constraint(equalTo: toolbar.topAnchor),
            done.bottomAnchor.constraint(equalTo: toolbar.bottomAnchor),
            done.widthAnchor.constraint(equalToConstant: 56)
        ])
        inputAccessoryView = toolbar
    }

    @objc private func hideKeyboard() { _ = resignFirstResponder() }

    override func layoutSubviews() { super.layoutSubviews(); idleCaret?.update() }
    override func didMoveToWindow() { super.didMoveToWindow(); idleCaret?.update() }
    override func becomeFirstResponder() -> Bool {
        let accepted = super.becomeFirstResponder()
        idleCaret?.update()
        return accepted
    }
    override func resignFirstResponder() -> Bool {
        let accepted = super.resignFirstResponder()
        idleCaret?.update()
        return accepted
    }

    // Only explicit model replacements can change UIKit's current text and selection.
    func replaceText(_ text: String, version: UInt64, attributes: [NSAttributedString.Key: Any]) {
        guard lastReplacement.map({ version > $0 }) ?? true, markedTextRange == nil else { return }
        lastReplacement = version
        guard self.text != text else { return }
        attributedText = NSAttributedString(string: text, attributes: attributes)
        typingAttributes = attributes
        selectedRange = NSRange(location: (text as NSString).length, length: 0)
    }

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
