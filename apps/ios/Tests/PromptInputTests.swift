// Check UIKit command admission and editing callbacks before exercising physical keys in the full app.
import UIKit
import XCTest
@testable import Carlos

final class PromptInputTests: XCTestCase {
    func testOldRendersCannotOverwriteNativeKeystrokesOrSelection() async {
        await MainActor.run {
            let input = PhonePromptTextView()
            let attributes: [NSAttributedString.Key: Any] = [.font: UIFont.monospacedSystemFont(ofSize: 14, weight: .regular)]
            input.replaceText("", version: 0, attributes: attributes)
            input.insertText("phone stream 🌍")
            input.selectedRange = NSRange(location: 5, length: 0)
            input.replaceText("phone", version: 0, attributes: attributes)
            XCTAssertEqual(input.text, "phone stream 🌍", "An older render cannot erase a space or newer keys")
            XCTAssertEqual(input.selectedRange.location, 5, "A redraw cannot move the insertion point")
            input.replaceText("", version: 1, attributes: attributes)
            XCTAssertEqual(input.text, "", "An explicit Send acknowledgement may clear the draft")
            input.insertText("new private text")
            input.replaceText("phone", version: 0, attributes: attributes)
            XCTAssertEqual(input.text, "new private text", "An older replacement cannot restore a sent prompt")
            input.replaceText("recovered 🦀", version: 2, attributes: attributes)
            XCTAssertEqual(input.text, "recovered 🦀", "Explicit recovery must still replace native text")
        }
    }

    func testNativeReturnPreservesTheDraftAndEditingDelegate() async {
        await MainActor.run {
            let input = PhonePromptTextView()
            let observer = PromptEdits()
            input.delegate = observer
            input.text = "phone 🌍"
            input.selectedRange = NSRange(location: (input.text as NSString).length, length: 0)
            XCTAssertTrue(input.handleReturn(modifiers: .shift))
            XCTAssertEqual(input.text, "phone 🌍\n")
            XCTAssertEqual(observer.text, input.text, "The private draft must receive the custom edit")
            var sent = 0
            input.send = { sent += 1 }
            XCTAssertTrue(input.handleReturn(modifiers: []))
            XCTAssertEqual(sent, 1)
            XCTAssertEqual(input.text, "phone 🌍\n")
        }
    }
}

@MainActor
private final class PromptEdits: NSObject, UITextViewDelegate {
    var text = ""
    func textViewDidChange(_ textView: UITextView) { text = textView.text }
}
