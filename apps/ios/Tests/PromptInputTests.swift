// Check UIKit command admission and editing callbacks before exercising physical keys in the full app.
import UIKit
import XCTest
@testable import Carlos

final class PromptInputTests: XCTestCase {
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
