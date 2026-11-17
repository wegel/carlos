// Keep the insertion point visible without making the editor open the software keyboard.
import UIKit

@MainActor
final class PhoneIdleCaret: NSObject {
    private weak var input: UITextView?
    private let caret = CALayer()
    private var blink: Task<Void, Never>?

    init(input: UITextView) {
        self.input = input
        super.init()
        input.layer.addSublayer(caret)
        NotificationCenter.default.addObserver(self, selector: #selector(update),
            name: UIAccessibility.reduceMotionStatusDidChangeNotification, object: nil)
        update()
    }

    deinit { blink?.cancel(); NotificationCenter.default.removeObserver(self) }

    @objc func update() {
        guard let input else { return }
        let hidden = input.isFirstResponder || input.window == nil
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        caret.isHidden = hidden
        if !hidden {
            caret.backgroundColor = UIColor(PhoneTheme.blue).cgColor
            caret.frame = input.caretRect(for: input.selectedTextRange?.end ?? input.endOfDocument)
        }
        CATransaction.commit()
        if hidden || UIAccessibility.isReduceMotionEnabled {
            blink?.cancel()
            blink = nil
            caret.opacity = 1
        } else if blink == nil {
            blink = Task { [weak self] in
                while !Task.isCancelled {
                    do { try await Task.sleep(for: .milliseconds(550)) }
                    catch { return }
                    guard !Task.isCancelled else { return }
                    self?.toggle()
                }
            }
        }
    }

    private func toggle() {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        caret.opacity = caret.opacity == 0 ? 1 : 0
        CATransaction.commit()
    }
}
