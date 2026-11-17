// Read the real cross-app clipboard while allowing the runner to answer iOS's Paste permission.
import UIKit
import XCTest

extension SessionWorkflowTests {
    @MainActor
    func copiedText(using button: XCUIElement) async -> String? {
        UIPasteboard.general.string = "The original has not been copied yet"
        button.tap()
        XCTAssertTrue(wait { button.label == "Copied" })
        let paste = Task.detached { UIPasteboard.general.string }
        let allow = XCUIApplication(bundleIdentifier: "com.apple.springboard").alerts.buttons["Allow Paste"]
        if allow.waitForExistence(timeout: 15) { allow.tap() }
        return await paste.value
    }
}
